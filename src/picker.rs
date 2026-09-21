use crate::buffer::{Edit, ShellBuffer};
use crate::schema::{SchemaDocument, ValueArity};
use std::borrow::Cow;
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionItem {
    pub flag: Cow<'static, str>,
    pub description: Cow<'static, str>,
}

pub const BUILTIN_OPTIONS: &[OptionItem] = &[
    OptionItem {
        flag: Cow::Borrowed("--help"),
        description: Cow::Borrowed("Show help information"),
    },
    OptionItem {
        flag: Cow::Borrowed("--verbose"),
        description: Cow::Borrowed("Enable verbose output"),
    },
    OptionItem {
        flag: Cow::Borrowed("--version"),
        description: Cow::Borrowed("Show version information"),
    },
];

pub fn options_from_schema(document: &SchemaDocument) -> Vec<OptionItem> {
    document
        .root
        .options
        .iter()
        .filter_map(|option| {
            let name = option.names.first()?;
            let mut flag = name.spelling();
            if !matches!(option.value.arity, ValueArity::None) {
                flag.push(' ');
            }
            Some(OptionItem {
                flag: Cow::Owned(flag),
                description: Cow::Owned(
                    option
                        .description
                        .clone()
                        .unwrap_or_else(|| "Discovered option".into()),
                ),
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertionPoint {
    Append,
    Cursor(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerOutcome {
    Confirm(Edit),
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PickerError {
    NoSelection,
    InvalidCursor { cursor: usize, buffer_len: usize },
}

impl fmt::Display for PickerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSelection => formatter.write_str("no option matches the current search"),
            Self::InvalidCursor { cursor, buffer_len } => write!(
                formatter,
                "cursor byte offset {cursor} is beyond buffer length {buffer_len}"
            ),
        }
    }
}

impl Error for PickerError {}

pub struct PickerState {
    options: Vec<OptionItem>,
    query: String,
    filtered_indices: Vec<usize>,
    selected: Option<usize>,
    buffer_len: usize,
    append_needs_space: bool,
}

impl PickerState {
    pub fn new(buffer: &ShellBuffer) -> Self {
        Self::with_options(buffer, BUILTIN_OPTIONS.to_vec())
    }

    pub fn with_options(buffer: &ShellBuffer, options: Vec<OptionItem>) -> Self {
        let text = buffer.text();
        let mut state = Self {
            options,
            query: String::new(),
            filtered_indices: Vec::new(),
            selected: None,
            buffer_len: text.len(),
            append_needs_space: !text.is_empty()
                && !text.chars().next_back().is_some_and(char::is_whitespace),
        };
        state.refilter();
        state
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn selected_item(&self) -> Option<&OptionItem> {
        self.selected
            .map(|position| &self.options[self.filtered_indices[position]])
    }

    pub fn selected_position(&self) -> Option<usize> {
        self.selected
    }

    pub fn filtered_items(&self) -> Vec<&OptionItem> {
        self.filtered_indices
            .iter()
            .map(|&index| &self.options[index])
            .collect()
    }

    pub fn move_next(&mut self) {
        if let Some(selected) = self.selected {
            self.selected = Some((selected + 1) % self.filtered_indices.len());
        }
    }

    pub fn move_previous(&mut self) {
        if let Some(selected) = self.selected {
            self.selected =
                Some((selected + self.filtered_indices.len() - 1) % self.filtered_indices.len());
        }
    }

    pub fn push_query_char(&mut self, character: char) {
        self.query.push(character);
        self.refilter();
    }

    pub fn pop_query_char(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn confirm(&self, insertion_point: InsertionPoint) -> Result<PickerOutcome, PickerError> {
        let item = self.selected_item().ok_or(PickerError::NoSelection)?;
        let (at, text) = match insertion_point {
            InsertionPoint::Append => (
                self.buffer_len,
                if self.append_needs_space {
                    format!(" {}", item.flag)
                } else {
                    item.flag.to_owned()
                },
            ),
            InsertionPoint::Cursor(cursor) => {
                if cursor > self.buffer_len {
                    return Err(PickerError::InvalidCursor {
                        cursor,
                        buffer_len: self.buffer_len,
                    });
                }
                (cursor, item.flag.to_owned())
            }
        };

        Ok(PickerOutcome::Confirm(Edit::Insert { at, text }))
    }

    pub fn cancel(&self) -> PickerOutcome {
        PickerOutcome::Cancel
    }

    fn refilter(&mut self) {
        let query = self.query.to_ascii_lowercase();
        self.filtered_indices = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                (item.flag.to_ascii_lowercase().contains(&query)
                    || item.description.to_ascii_lowercase().contains(&query))
                .then_some(index)
            })
            .collect();
        self.selected = (!self.filtered_indices.is_empty()).then_some(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str) -> PickerState {
        PickerState::new(&ShellBuffer::new(text))
    }

    #[test]
    fn options_have_deterministic_order() {
        assert_eq!(
            BUILTIN_OPTIONS
                .iter()
                .map(|item| item.flag)
                .collect::<Vec<_>>(),
            vec!["--help", "--verbose", "--version"]
        );
    }

    #[test]
    fn search_matches_names_descriptions_and_ascii_case() {
        let mut picker = state("");
        picker.push_query_char('v');
        assert_eq!(picker.selected_item().unwrap().flag, "--verbose");

        picker.pop_query_char();
        for character in "OUTPUT".chars() {
            picker.push_query_char(character);
        }
        assert_eq!(picker.selected_item().unwrap().flag, "--verbose");
    }

    #[test]
    fn no_matches_has_no_selection_and_cannot_confirm() {
        let mut picker = state("");
        picker.push_query_char('z');
        assert!(picker.selected_item().is_none());
        assert_eq!(
            picker.confirm(InsertionPoint::Append),
            Err(PickerError::NoSelection)
        );
    }

    #[test]
    fn navigation_wraps_in_both_directions() {
        let mut picker = state("");
        picker.move_previous();
        assert_eq!(picker.selected_item().unwrap().flag, "--version");
        picker.move_next();
        assert_eq!(picker.selected_item().unwrap().flag, "--help");
    }

    #[test]
    fn query_deletion_restores_matches() {
        let mut picker = state("");
        for character in "help".chars() {
            picker.push_query_char(character);
        }
        assert_eq!(picker.filtered_items().len(), 1);
        picker.pop_query_char();
        assert_eq!(picker.query(), "hel");
        assert_eq!(picker.selected_item().unwrap().flag, "--help");
    }

    #[test]
    fn append_spacing_is_conservative() {
        let empty = state("");
        assert_eq!(
            empty.confirm(InsertionPoint::Append),
            Ok(PickerOutcome::Confirm(Edit::Insert {
                at: 0,
                text: "--help".into()
            }))
        );

        let nonempty = state("git");
        assert_eq!(
            nonempty.confirm(InsertionPoint::Append),
            Ok(PickerOutcome::Confirm(Edit::Insert {
                at: 3,
                text: " --help".into()
            }))
        );

        let trailing_whitespace = state("git ");
        assert_eq!(
            trailing_whitespace.confirm(InsertionPoint::Append),
            Ok(PickerOutcome::Confirm(Edit::Insert {
                at: 4,
                text: "--help".into()
            }))
        );
    }

    #[test]
    fn cursor_insertion_does_not_add_spacing() {
        let picker = state("git status");
        assert_eq!(
            picker.confirm(InsertionPoint::Cursor(2)),
            Ok(PickerOutcome::Confirm(Edit::Insert {
                at: 2,
                text: "--help".into()
            }))
        );
    }

    #[test]
    fn partial_command_confirmation_returns_the_edited_buffer() {
        let buffer = ShellBuffer::new("curl https://example.com");
        let mut picker = PickerState::new(&buffer);
        for character in "verbose".chars() {
            picker.push_query_char(character);
        }

        let PickerOutcome::Confirm(edit) = picker
            .confirm(InsertionPoint::Append)
            .expect("matching option can be confirmed")
        else {
            panic!("confirmation must return an edit");
        };

        assert_eq!(
            buffer.apply(&[edit]),
            Ok("curl https://example.com --verbose".to_owned())
        );
    }

    #[test]
    fn cancellation_returns_no_edit() {
        assert_eq!(state("curl").cancel(), PickerOutcome::Cancel);
    }

    #[test]
    fn discovered_schema_options_replace_the_builtin_list() {
        let document =
            SchemaDocument::from_json(include_str!("../tests/fixtures/git-schema-v1.json"))
                .expect("schema fixture parses");
        let options = options_from_schema(&document);
        let mut picker = PickerState::with_options(&ShellBuffer::new("git"), options);
        for character in "verbose".chars() {
            picker.push_query_char(character);
        }
        assert_eq!(
            picker.selected_item().map(|item| item.flag.as_ref()),
            Some("--verbose")
        );
    }
}
