use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

fn fixture_command(label: &str) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/discovery-help.sh");
    let target =
        std::env::temp_dir().join(format!("flagpick-discovery-{}-{label}", std::process::id()));
    fs::copy(source, &target).expect("copy discovery fixture");
    let mut permissions = fs::metadata(&target)
        .expect("fixture metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&target, permissions).expect("make fixture executable");
    target
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flagpick"))
        .args(args)
        .output()
        .expect("run flagpick")
}

#[test]
fn schema_emits_valid_canonical_json_for_an_explicit_executable() {
    let executable = fixture_command("schema");
    let output = run(&[
        "schema",
        executable.to_str().expect("fixture path is UTF-8"),
        "--format",
        "json",
    ]);
    let _ = fs::remove_file(executable);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).expect("valid JSON");
    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["root"]["metadata"]["source"], "generic_help");
    assert!(output.stderr.is_empty());
}

#[test]
fn inspect_exposes_probe_parser_and_cache_diagnostics() {
    let executable = fixture_command("inspect");
    let output = run(&[
        "inspect",
        executable.to_str().expect("fixture path is UTF-8"),
    ]);
    let _ = fs::remove_file(executable);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("valid JSON");
    for field in [
        "executable",
        "argv",
        "parser",
        "source",
        "confidence",
        "cache",
        "probe",
        "sanitizer",
    ] {
        assert!(
            diagnostics.get(field).is_some(),
            "missing {field}: {diagnostics}"
        );
    }
    assert_eq!(diagnostics["cache"], "miss");
}

#[test]
fn inspect_rejects_extra_flags_instead_of_executing_them() {
    let output = run(&["inspect", "echo", "-n", "dangerous"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("only explicit subcommand names"));
}

#[test]
fn no_exec_probe_never_spawns_an_explicit_target() {
    let executable = fixture_command("no-exec");
    let output = run(&[
        "inspect",
        "--no-exec-probe",
        executable.to_str().expect("fixture path is UTF-8"),
    ]);
    let _ = fs::remove_file(executable);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no cached schema"));
}

#[test]
fn unavailable_commands_fail_without_stdout() {
    let output = run(&["inspect", "flagpick-command-that-does-not-exist"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unavailable"));
}
