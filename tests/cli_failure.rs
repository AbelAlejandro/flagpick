use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_flagpick"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn flagpick binary");

    {
        let mut child_stdin = child.stdin.take().expect("flagpick stdin pipe");
        child_stdin
            .write_all(input)
            .expect("write to flagpick stdin");
    }

    child.wait_with_output().expect("wait for flagpick")
}

fn assert_failure_with_empty_stdout(output: &Output) {
    assert!(
        !output.status.success(),
        "command should fail, but exited with {:?}",
        output.status
    );
    assert!(
        output.stdout.is_empty(),
        "failed command must not write a replacement buffer, got {:?}",
        output.stdout
    );
}

#[test]
fn missing_arguments_fail_with_empty_stdout() {
    let output = run(&[], b"");
    assert_failure_with_empty_stdout(&output);
}

#[test]
fn unknown_shell_fails_with_empty_stdout() {
    let output = run(
        &["shell-edit", "definitely-not-a-shell", "--cursor", "0"],
        b"",
    );
    assert_failure_with_empty_stdout(&output);
}

#[test]
fn out_of_range_unicode_cursor_fails_with_empty_stdout() {
    let output = run(&["shell-edit", "zsh", "--cursor", "3"], "aé".as_bytes());
    assert_failure_with_empty_stdout(&output);
}

#[test]
fn valid_invocation_with_piped_stdio_never_emits_buffer() {
    if std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .is_ok()
    {
        eprintln!("skipping: /dev/tty is available in this environment");
        return;
    }

    let output = run(&["shell-edit", "zsh", "--cursor", "1"], b"abc");
    assert_failure_with_empty_stdout(&output);
}

#[test]
fn oversized_buffer_fails_before_opening_the_tui() {
    let input = vec![b'x'; 1024 * 1024 + 1];
    let output = run(&["shell-edit", "zsh", "--cursor", "0"], input.as_slice());
    assert_failure_with_empty_stdout(&output);
}
