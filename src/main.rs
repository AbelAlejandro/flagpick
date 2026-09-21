mod ui;

use flagpick::buffer::ShellBuffer;
use flagpick::character_cursor_to_byte;
use flagpick::discovery;
use flagpick::picker::{InsertionPoint, PickerOutcome};
use std::env;
use std::error::Error;
use std::io::{self, Read, Write};
use std::process::ExitCode;

const EXIT_CANCELLED: u8 = 130;
const MAX_BUFFER_BYTES: u64 = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
struct ShellEditArgs {
    cursor: usize,
    at_cursor: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(Some(output)) => {
            if let Err(error) = io::stdout().write_all(output.as_bytes()) {
                fail(error)
            } else {
                ExitCode::SUCCESS
            }
        }
        Ok(None) => ExitCode::from(EXIT_CANCELLED),
        Err(error) => fail(error),
    }
}

fn run() -> Result<Option<String>, Box<dyn Error>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match arguments.first().map(String::as_str) {
        Some("inspect") => {
            return run_discovery(arguments[1..].to_vec(), false)
                .map(Some)
                .map_err(Into::into);
        }
        Some("schema") => {
            return run_discovery(arguments[1..].to_vec(), true)
                .map(Some)
                .map_err(Into::into);
        }
        _ => {}
    }
    let args = parse_shell_edit_args(env::args().skip(1))?;
    let mut input = String::new();
    let mut limited = io::stdin().take(MAX_BUFFER_BYTES + 1);
    limited.read_to_string(&mut input)?;
    if input.len() as u64 > MAX_BUFFER_BYTES {
        return Err("shell buffer exceeds the 1 MiB safety limit".into());
    }
    let cursor = character_cursor_to_byte(&input, args.cursor)?;
    let insertion_point = if args.at_cursor {
        InsertionPoint::Cursor(cursor)
    } else {
        InsertionPoint::Append
    };
    let buffer = ShellBuffer::new(input);
    match ui::run_picker(&buffer, insertion_point)? {
        PickerOutcome::Confirm(edit) => Ok(Some(buffer.apply(&[edit])?)),
        PickerOutcome::Cancel => Ok(None),
    }
}

fn run_discovery(arguments: Vec<String>, schema_command: bool) -> Result<String, String> {
    let mut no_exec_probe = false;
    let mut format_json = false;
    let mut command = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--no-exec-probe" => no_exec_probe = true,
            "--format" => {
                index += 1;
                if arguments.get(index).map(String::as_str) != Some("json") {
                    return Err("--format currently accepts only json".into());
                }
                format_json = true;
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown discovery argument: {value}"));
            }
            value => command.push(value.to_owned()),
        }
        index += 1;
    }
    if schema_command && !format_json {
        return Err("schema requires --format json".into());
    }
    let result =
        discovery::discover(&command, !no_exec_probe).map_err(|error| error.to_string())?;
    if schema_command {
        return result
            .document
            .to_canonical_json()
            .map_err(|error| error.to_string());
    }
    let warnings = result
        .warnings
        .into_iter()
        .map(|warning| serde_json::json!({ "line": warning.line, "message": warning.message }))
        .collect::<Vec<_>>();
    let diagnostics = serde_json::json!({
        "executable": result.executable,
        "argv": result.argv,
        "parser": result.document.root.metadata.parser,
        "source": result.document.root.metadata.source,
        "confidence": result.document.root.metadata.confidence,
        "cache": result.cache.as_str(),
        "probe": result
            .probe
            .map_or_else(|| serde_json::json!({ "source": "cache" }), |probe| serde_json::json!(probe)),
        "sanitizer": "utf8-control-v1",
        "warnings": warnings,
        "schema": result.document,
    });
    serde_json::to_string_pretty(&diagnostics).map_err(|error| error.to_string())
}

fn parse_shell_edit_args(mut args: impl Iterator<Item = String>) -> Result<ShellEditArgs, String> {
    if args.next().as_deref() != Some("shell-edit") || args.next().as_deref() != Some("zsh") {
        return Err(usage());
    }

    let mut cursor = None;
    let mut at_cursor = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--cursor" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--cursor requires a value".to_owned())?;
                cursor = Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| "--cursor must be a non-negative integer".to_owned())?,
                );
            }
            "--at-cursor" => at_cursor = true,
            _ => return Err(format!("unknown argument: {argument}\n\n{}", usage())),
        }
    }

    Ok(ShellEditArgs {
        cursor: cursor.ok_or_else(|| "--cursor is required".to_owned())?,
        at_cursor,
    })
}

fn usage() -> String {
    "Usage: flagpick shell-edit zsh --cursor <character-position> [--at-cursor]\n       flagpick inspect [--no-exec-probe] <command...>\n       flagpick schema <command...> --format json".to_owned()
}

fn fail(error: impl std::fmt::Display) -> ExitCode {
    eprintln!("flagpick: {error}");
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use super::{ShellEditArgs, parse_shell_edit_args};

    fn parse(args: &[&str]) -> Result<ShellEditArgs, String> {
        parse_shell_edit_args(args.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn parses_append_and_cursor_modes() {
        assert_eq!(
            parse(&["shell-edit", "zsh", "--cursor", "4"]),
            Ok(ShellEditArgs {
                cursor: 4,
                at_cursor: false,
            })
        );
        assert_eq!(
            parse(&["shell-edit", "zsh", "--at-cursor", "--cursor", "2",]),
            Ok(ShellEditArgs {
                cursor: 2,
                at_cursor: true,
            })
        );
    }

    #[test]
    fn rejects_missing_or_unknown_arguments() {
        assert!(parse(&["shell-edit", "zsh"]).is_err());
        assert!(parse(&["shell-edit", "bash", "--cursor", "0"]).is_err());
        assert!(parse(&["shell-edit", "zsh", "--cursor", "-1"]).is_err());
        assert!(parse(&["shell-edit", "zsh", "--wat"]).is_err());
    }
}
