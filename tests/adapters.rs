use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture_dir(name: &str, fixture: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "flagpick-com70-{name}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&directory).expect("create fixture directory");
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture);
    let executable = directory.join(name);
    fs::copy(source, &executable).expect("copy fixture");
    let mut permissions = fs::metadata(&executable)
        .expect("fixture metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("make fixture executable");
    directory
}

fn run(arguments: &[&str], directory: &Path) -> Output {
    let path = format!(
        "{}:{}",
        directory.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(env!("CARGO_BIN_EXE_flagpick"))
        .args(arguments)
        .env("PATH", path)
        .output()
        .expect("run flagpick")
}

fn schema(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("schema JSON")
}

#[test]
fn git_adapters_preserve_subcommand_context_and_metadata() {
    for (subcommand, expected_option) in [("commit", "--message"), ("remote", "--verbose")] {
        let directory = fixture_dir("git", "adapter-git.sh");
        assert!(
            Command::new(directory.join("git"))
                .arg(subcommand)
                .status()
                .expect("run fixture")
                .success()
        );
        let executable = directory.join("git");
        let executable = executable.to_str().expect("fixture path is UTF-8");
        let output = run(
            &["schema", executable, subcommand, "--format", "json"],
            &directory,
        );
        let document = schema(&output);
        let root = &document["root"];
        assert_eq!(root["name"], "git");
        assert_eq!(root["subcommands"][0]["name"], subcommand);
        assert!(
            root["subcommands"][0]["command"]["options"]
                .to_string()
                .contains(expected_option.trim_start_matches('-'))
        );
        assert_eq!(root["metadata"]["source"], "framework_help");
        assert_eq!(root["metadata"]["parser"], "git-help-v1");
        assert_eq!(root["metadata"]["confidence"], "high");
        let _ = fs::remove_dir_all(directory);
    }
}

#[test]
fn curl_adapter_uses_extended_help_probe() {
    let directory = fixture_dir("curl", "adapter-curl.sh");
    let executable = directory.join("curl");
    let executable = executable.to_str().expect("fixture path is UTF-8");
    let output = run(&["inspect", executable], &directory);
    let diagnostics = schema(&output);
    assert_eq!(diagnostics["argv"], serde_json::json!(["--help", "all"]));
    assert_eq!(diagnostics["parser"], "curl-help-v1");
    assert_eq!(diagnostics["source"], "framework_help");
    assert!(
        diagnostics["schema"]["root"]["options"]
            .to_string()
            .contains("--proxy")
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn unsupported_subcommand_is_rejected_without_probe() {
    let directory = fixture_dir("git", "adapter-git.sh");
    let output = run(&["inspect", "git", "status"], &directory);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported command path"));
    let _ = fs::remove_dir_all(directory);
}
