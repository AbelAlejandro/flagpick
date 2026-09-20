use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use flagpick::buffer::ShellBuffer;
use flagpick::picker::{InsertionPoint, PickerError, PickerOutcome, PickerState};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use std::fs::{File, OpenOptions};
use std::io;
use std::thread;
use std::time::Duration;

const MAX_TERMINAL_DISPLAY_LENGTH: usize = 4096;

fn consume_csi(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(character) = characters.next() {
        if ('@'..='~').contains(&character) {
            break;
        }
    }
}

fn consume_string(characters: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(character) = characters.next() {
        match character {
            '\x07' | '\u{9c}' => break,
            '\x1b' if characters.next_if_eq(&'\\').is_some() => break,
            _ => {}
        }
    }
}

pub fn run_picker(
    buffer: &ShellBuffer,
    insertion_point: InsertionPoint,
) -> Result<PickerOutcome, UiError> {
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut input = tty.try_clone()?;
    let mut terminal = TerminalSession::new(tty)?;
    let mut state = PickerState::new(buffer);

    loop {
        terminal.draw(&state, buffer, insertion_point)?;
        match read_key(&mut input)? {
            InputKey::Cancel => return Ok(state.cancel()),
            InputKey::Confirm => return state.confirm(insertion_point).map_err(UiError::Picker),
            InputKey::Previous => state.move_previous(),
            InputKey::Next => state.move_next(),
            InputKey::Backspace => state.pop_query_char(),
            InputKey::Character(character) => state.push_query_char(character),
            InputKey::Ignore => {}
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InputKey {
    Cancel,
    Confirm,
    Previous,
    Next,
    Backspace,
    Character(char),
    Ignore,
}

fn read_key(input: &mut File) -> io::Result<InputKey> {
    let mut first = [0_u8; 1];
    std::io::Read::read_exact(input, &mut first)?;

    if first[0] == 0x1b {
        for _ in 0..5 {
            let available = rustix::io::ioctl_fionread(&*input).map_err(io::Error::from)?;
            if available >= 2 {
                let mut rest = [0_u8; 2];
                std::io::Read::read_exact(input, &mut rest)?;
                return Ok(decode_key(&[first[0], rest[0], rest[1]]));
            }
            thread::sleep(Duration::from_millis(2));
        }
        return Ok(InputKey::Cancel);
    }

    let width = utf8_width(first[0]);
    if width == 0 {
        return Ok(decode_key(&first));
    }
    let mut bytes = vec![first[0]];
    bytes.resize(width, 0);
    std::io::Read::read_exact(input, &mut bytes[1..])?;
    Ok(decode_key(&bytes))
}

fn utf8_width(first: u8) -> usize {
    match first {
        0x00..=0x7f => 0,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => 0,
    }
}

fn decode_key(bytes: &[u8]) -> InputKey {
    match bytes {
        [0x03] | [0x1b] => InputKey::Cancel,
        [b'\r'] | [b'\n'] => InputKey::Confirm,
        [0x08] | [0x7f] => InputKey::Backspace,
        [0x1b, b'[', b'A'] | [0x1b, b'O', b'A'] => InputKey::Previous,
        [0x1b, b'[', b'B'] | [0x1b, b'O', b'B'] => InputKey::Next,
        [byte] if byte.is_ascii_graphic() || *byte == b' ' => {
            InputKey::Character(char::from(*byte))
        }
        _ => std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| text.chars().next())
            .map_or(InputKey::Ignore, InputKey::Character),
    }
}

#[derive(Debug)]
pub enum UiError {
    Io(io::Error),
    Picker(PickerError),
}

impl std::fmt::Display for UiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Picker(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for UiError {}

impl From<io::Error> for UiError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<File>>,
    cleanup: File,
}

impl TerminalSession {
    fn new(mut tty: File) -> io::Result<Self> {
        let mut cleanup = tty.try_clone()?;
        enable_raw_mode()?;
        if let Err(error) = execute!(tty, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }

        match Terminal::new(CrosstermBackend::new(tty)) {
            Ok(terminal) => Ok(Self { terminal, cleanup }),
            Err(error) => {
                let _ = execute!(cleanup, LeaveAlternateScreen);
                let _ = disable_raw_mode();
                Err(error)
            }
        }
    }

    fn draw(
        &mut self,
        state: &PickerState,
        buffer: &ShellBuffer,
        insertion_point: InsertionPoint,
    ) -> io::Result<()> {
        self.terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(3),
                    Constraint::Length(3),
                    Constraint::Length(1),
                ])
                .split(frame.area());

            let search = Paragraph::new(state.query()).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Flagpick — Search"),
            );
            frame.render_widget(search, chunks[0]);

            let items = state
                .filtered_items()
                .into_iter()
                .map(|item| {
                    ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{:<14}", item.flag),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(item.description),
                    ]))
                })
                .collect::<Vec<_>>();
            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Flags"))
                .highlight_symbol("> ")
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
            let mut list_state = ListState::default().with_selected(state.selected_position());
            frame.render_stateful_widget(list, chunks[1], &mut list_state);

            let preview = state
                .confirm(insertion_point)
                .ok()
                .and_then(|outcome| match outcome {
                    PickerOutcome::Confirm(edit) => buffer.apply(&[edit]).ok(),
                    PickerOutcome::Cancel => None,
                })
                .unwrap_or_else(|| buffer.text().to_owned());
            frame.render_widget(
                Paragraph::new(sanitize_for_terminal(&preview))
                    .block(Block::default().borders(Borders::ALL).title("Result")),
                chunks[2],
            );
            frame.render_widget(
                Paragraph::new("↑/↓ move   Enter insert   Esc cancel"),
                chunks[3],
            );
        })?;
        Ok(())
    }
}

fn sanitize_for_terminal(text: &str) -> String {
    let mut sanitized = String::new();
    let mut characters = text.chars().peekable();
    let mut display_length = 0;

    while let Some(character) = characters.next() {
        if display_length == MAX_TERMINAL_DISPLAY_LENGTH {
            break;
        }

        match character {
            '\x1b' => match characters.next_if_eq(&'[') {
                Some(_) => consume_csi(&mut characters),
                None => match characters.next_if_eq(&']') {
                    Some(_) => consume_string(&mut characters),
                    None => match characters.next_if_eq(&'P') {
                        Some(_) => consume_string(&mut characters),
                        None => match characters.next_if_eq(&'_') {
                            Some(_) => consume_string(&mut characters),
                            None => match characters.next_if_eq(&'^') {
                                Some(_) => consume_string(&mut characters),
                                None => {
                                    sanitized.push('�');
                                    display_length += 1;
                                }
                            },
                        },
                    },
                },
            },
            '\u{9b}' => consume_csi(&mut characters),
            '\u{90}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => {
                consume_string(&mut characters)
            }
            '\n' | '\t' => {
                sanitized.push(character);
                display_length += 1;
            }
            _ if character.is_control() => {
                sanitized.push('�');
                display_length += 1;
            }
            _ => {
                sanitized.push(character);
                display_length += 1;
            }
        }
    }

    sanitized
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = execute!(self.cleanup, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = self.terminal.show_cursor();
    }
}

#[cfg(test)]
mod tests {
    use super::{InputKey, decode_key, sanitize_for_terminal, utf8_width};

    #[test]
    fn decodes_navigation_confirmation_and_cancellation() {
        assert_eq!(decode_key(b"\x1b[A"), InputKey::Previous);
        assert_eq!(decode_key(b"\x1b[B"), InputKey::Next);
        assert_eq!(decode_key(b"\r"), InputKey::Confirm);
        assert_eq!(decode_key(b"\x1b"), InputKey::Cancel);
        assert_eq!(decode_key(b"\x03"), InputKey::Cancel);
        assert_eq!(decode_key(b"\x7f"), InputKey::Backspace);
    }

    #[test]
    fn decodes_ascii_and_unicode_search_input() {
        assert_eq!(decode_key(b"v"), InputKey::Character('v'));
        assert_eq!(decode_key("雪".as_bytes()), InputKey::Character('雪'));
        assert_eq!(utf8_width("雪".as_bytes()[0]), 3);
        assert_eq!(decode_key(&[0xff]), InputKey::Ignore);
    }

    #[test]
    fn neutralizes_terminal_controls_in_the_preview_only() {
        assert_eq!(
            sanitize_for_terminal("safe\n\x1b]52;clipboard\x07\tend"),
            "safe\n\tend"
        );
    }

    #[test]
    fn strips_ansi_sequences_including_unterminated_sequences() {
        assert_eq!(
            sanitize_for_terminal(
                "\x1b[31mred\x1b]52;clipboard\x07\x1bPsecret\x1b\\\
\x1b_hidden\x1b\\\x1b^private\x1b\\\x1b]unterminated\u{9b}red"
            ),
            "red"
        );
    }

    #[test]
    fn strips_seven_bit_string_controls_with_bel_and_string_terminators() {
        assert_eq!(
            sanitize_for_terminal(
                "left\x1bP dcs\x1b\\\x1b]osc\x07\x1b]osc-st\x1b\\\x1b^pm\x1b\\\x1b_apc\x1b\\right"
            ),
            "leftright"
        );
    }

    #[test]
    fn strips_c1_string_controls_and_preserves_lone_escape_safely() {
        assert_eq!(
            sanitize_for_terminal(
                "\u{90}dcs\u{9c}\u{9d}oscbell\x07\u{9e}pm\x1b\\\u{9f}apc\u{9c}"
            ),
            ""
        );
        assert_eq!(sanitize_for_terminal("before\x1bafter"), "before�after");
    }

    #[test]
    fn replaces_malformed_utf8_lossy_input() {
        let text = String::from_utf8_lossy(b"safe\xff\xfe").into_owned();

        assert_eq!(sanitize_for_terminal(&text), "safe��");
    }

    #[test]
    fn caps_the_display_length() {
        let text = "x".repeat(MAX_TERMINAL_DISPLAY_LENGTH + 10);

        assert_eq!(
            sanitize_for_terminal(&text),
            "x".repeat(MAX_TERMINAL_DISPLAY_LENGTH)
        );
    }
}
