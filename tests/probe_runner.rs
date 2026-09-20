use std::env;
use std::ffi::{OsStr, OsString};
#[cfg(unix)]
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{self, Command};
#[cfg(unix)]
use std::thread;
use std::time::Duration;

use flagpick::probe::{ProbeConfig, ProbeError, ProbeRequest, ProbeRunner};

#[cfg(unix)]
use rustix::process::{Pid, Signal, getpid, getppid, kill_process, setsid};

fn helper_mode(name: &str) -> bool {
    let args = env::args_os().collect::<Vec<_>>();
    args.iter().any(|arg| arg == OsStr::new("--exact"))
        && args.iter().any(|arg| arg == OsStr::new(name))
}

fn helper_argv(name: &str) -> Vec<OsString> {
    ["--exact", name, "--nocapture", "--test-threads=1"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn valid_config() -> ProbeConfig {
    ProbeConfig {
        execution_enabled: true,
        timeout: Duration::from_secs(5),
        stdout_cap: 8192,
        stderr_cap: 8192,
    }
}

#[test]
fn probe_helper_output() -> io::Result<()> {
    if !helper_mode("probe_helper_output") {
        return Ok(());
    }
    let mut stdout = io::stdout();
    stdout.write_all(&[0xfd, 0xfe, 0xff])?;
    let argv = env::args_os()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    writeln!(stdout, "PROBE_ARGV={argv}")?;
    for _ in 0..20 {
        stdout.write_all(&[b'b'; 256])?;
    }
    stdout.flush()?;

    let mut stderr = io::stderr();
    stderr.write_all(&[0xfd, 0xfe, 0xff])?;
    for _ in 0..20 {
        stderr.write_all(&[b'c'; 256])?;
    }
    stderr.flush()
}

#[test]
fn probe_helper_env() -> Result<(), Box<dyn std::error::Error>> {
    if !helper_mode("probe_helper_env") {
        return Ok(());
    }
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input)?;
    println!("STDIN_LEN={}", input.len());
    for variable in ["TERM", "NO_COLOR", "PAGER", "GIT_PAGER", "MANPAGER"] {
        println!(
            "{variable}={}",
            env::var(variable).unwrap_or_else(|_| "<unset>".into())
        );
    }
    println!(
        "HOME={}",
        if env::var_os("HOME").is_some() {
            "present"
        } else {
            "<unset>"
        }
    );
    io::stdout().flush()?;
    Ok(())
}

#[test]
fn probe_helper_nonzero() {
    if helper_mode("probe_helper_nonzero") {
        process::exit(23);
    }
}

#[cfg(unix)]
#[test]
fn probe_helper_signal() -> Result<(), Box<dyn std::error::Error>> {
    if helper_mode("probe_helper_signal") {
        kill_process(getpid(), Signal::TERM)?;
    }
    Ok(())
}

#[cfg(unix)]
fn marker_path(pid: i32) -> PathBuf {
    PathBuf::from(format!("/tmp/flagpick-probe-descendant-{pid}"))
}

#[cfg(unix)]
#[test]
fn probe_helper_timeout_parent() -> Result<(), Box<dyn std::error::Error>> {
    if !helper_mode("probe_helper_timeout_parent") {
        return Ok(());
    }
    let pid = getpid().as_raw_nonzero().get();
    println!("PROBE_PARENT_PID={pid}");
    io::stdout().flush()?;
    let marker = marker_path(pid);
    let _ = fs::remove_file(&marker);
    let _child = Command::new(env::current_exe()?)
        .args(helper_argv("probe_helper_timeout_descendant"))
        .spawn()?;
    thread::sleep(Duration::from_secs(10));
    Ok(())
}

#[cfg(unix)]
#[test]
fn probe_helper_timeout_descendant() -> Result<(), Box<dyn std::error::Error>> {
    if !helper_mode("probe_helper_timeout_descendant") {
        return Ok(());
    }
    let parent = getppid().ok_or("no parent pid")?.as_raw_nonzero().get();
    thread::sleep(Duration::from_millis(1500));
    fs::write(marker_path(parent), b"descendant-alive")?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn probe_helper_escaped_parent() -> Result<(), Box<dyn std::error::Error>> {
    if !helper_mode("probe_helper_escaped_parent") {
        return Ok(());
    }
    let _child = Command::new(env::current_exe()?)
        .args(helper_argv("probe_helper_escaped_descendant"))
        .spawn()?;
    thread::sleep(Duration::from_secs(10));
    Ok(())
}

#[cfg(unix)]
#[test]
fn probe_helper_escaped_descendant() -> Result<(), Box<dyn std::error::Error>> {
    if !helper_mode("probe_helper_escaped_descendant") {
        return Ok(());
    }
    setsid()?;
    println!("ESCAPED_PID={}", getpid().as_raw_nonzero().get());
    io::stdout().flush()?;
    thread::sleep(Duration::from_secs(10));
    Ok(())
}

#[cfg(unix)]
fn parse_parent_pid(stdout: &str) -> Result<i32, Box<dyn std::error::Error>> {
    let marker = "PROBE_PARENT_PID=";
    let start = stdout
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| format!("missing parent PID in captured output: {stdout}"))?;
    let digits = stdout[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    Ok(digits.parse()?)
}

#[cfg(unix)]
fn parse_escaped_pid(stdout: &str) -> Result<Pid, Box<dyn std::error::Error>> {
    let marker = "ESCAPED_PID=";
    let start = stdout
        .find(marker)
        .map(|index| index + marker.len())
        .ok_or_else(|| format!("missing escaped PID in captured output: {stdout}"))?;
    let digits = stdout[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    Pid::from_raw(digits.parse()?).ok_or_else(|| "escaped PID was zero".into())
}

#[test]
fn disabled_execution_returns_before_spawn() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(ProbeConfig::default())?;
    let request = ProbeRequest::new(
        PathBuf::from("/definitely/not/a/real/flagpick/binary"),
        vec![OsString::from("--help")],
    )?;
    assert!(matches!(
        runner.run(&request),
        Err(ProbeError::ExecutionDisabled)
    ));
    Ok(())
}

#[test]
fn invalid_config_and_request_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    for config in [
        ProbeConfig {
            timeout: Duration::ZERO,
            ..valid_config()
        },
        ProbeConfig {
            stdout_cap: 0,
            ..valid_config()
        },
        ProbeConfig {
            stderr_cap: 0,
            ..valid_config()
        },
        ProbeConfig {
            timeout: Duration::from_secs(u64::MAX),
            ..valid_config()
        },
        ProbeConfig {
            stdout_cap: usize::MAX,
            ..valid_config()
        },
        ProbeConfig {
            stderr_cap: usize::MAX,
            ..valid_config()
        },
    ] {
        assert!(matches!(
            ProbeRunner::new(config),
            Err(ProbeError::InvalidConfig(_))
        ));
    }

    assert!(matches!(
        ProbeRequest::new(PathBuf::new(), vec![OsString::from("--help")]),
        Err(ProbeError::InvalidRequest(_))
    ));
    assert!(matches!(
        ProbeRequest::new(PathBuf::from("relative"), vec![OsString::from("--help")]),
        Err(ProbeError::InvalidRequest(_))
    ));
    assert!(matches!(
        ProbeRequest::new(env::current_exe()?, vec![OsString::new()]),
        Err(ProbeError::EmptyArgument(0))
    ));
    Ok(())
}

#[test]
fn exact_argv_is_observable() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(valid_config())?;
    let request = ProbeRequest::new(env::current_exe()?, helper_argv("probe_helper_output"))?;
    let output = runner.run(&request)?;
    let stdout = String::from_utf8_lossy(&output.stdout.bytes);
    assert!(stdout.contains("--exact probe_helper_output --nocapture --test-threads=1"));
    assert_eq!(output.argv, helper_argv("probe_helper_output"));
    Ok(())
}

#[test]
fn controlled_environment_and_null_stdin_are_observable() -> Result<(), Box<dyn std::error::Error>>
{
    let runner = ProbeRunner::new(valid_config())?;
    let request = ProbeRequest::new(env::current_exe()?, helper_argv("probe_helper_env"))?;
    let output = runner.run(&request)?;
    let stdout = String::from_utf8_lossy(&output.stdout.bytes);
    for expected in [
        "STDIN_LEN=0",
        "TERM=dumb",
        "NO_COLOR=1",
        "PAGER=cat",
        "GIT_PAGER=cat",
        "MANPAGER=cat",
        "HOME=<unset>",
    ] {
        assert!(stdout.contains(expected), "missing {expected} in {stdout}");
    }
    Ok(())
}

#[test]
fn streams_are_capped_drained_and_preserve_raw_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(ProbeConfig {
        execution_enabled: true,
        timeout: Duration::from_secs(5),
        stdout_cap: 512,
        stderr_cap: 64,
    })?;
    let request = ProbeRequest::new(env::current_exe()?, helper_argv("probe_helper_output"))?;
    let output = runner.run(&request)?;

    assert_eq!(output.stdout.bytes.len(), 512);
    assert!(output.stdout.total_bytes > 512 && output.stdout.truncated);
    assert!(
        output
            .stdout
            .bytes
            .windows(3)
            .any(|window| window == [0xfd, 0xfe, 0xff])
    );
    assert_eq!(output.stderr.bytes.len(), 64);
    assert!(output.stderr.total_bytes > 64 && output.stderr.truncated);
    assert!(output.stderr.bytes.starts_with(&[0xfd, 0xfe, 0xff]));
    Ok(())
}

#[test]
fn nonzero_exit_is_output_data() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(valid_config())?;
    let request = ProbeRequest::new(env::current_exe()?, helper_argv("probe_helper_nonzero"))?;
    let output = runner.run(&request)?;
    assert_eq!(output.status.code, Some(23));
    assert!(!output.timed_out);
    Ok(())
}

#[cfg(unix)]
#[test]
fn signal_status_is_output_data() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(valid_config())?;
    let request = ProbeRequest::new(env::current_exe()?, helper_argv("probe_helper_signal"))?;
    let output = runner.run(&request)?;
    assert_eq!(output.status.signal, Some(15));
    assert!(!output.timed_out);
    Ok(())
}

#[cfg(unix)]
#[test]
fn timeout_kills_process_group_and_descendant() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(ProbeConfig {
        execution_enabled: true,
        timeout: Duration::from_millis(750),
        stdout_cap: 4096,
        stderr_cap: 4096,
    })?;
    let request = ProbeRequest::new(
        env::current_exe()?,
        helper_argv("probe_helper_timeout_parent"),
    )?;
    let output = runner.run(&request)?;
    assert!(output.timed_out);
    assert!(output.duration < Duration::from_secs(2));

    let parent_pid = parse_parent_pid(&String::from_utf8_lossy(&output.stdout.bytes))?;
    let marker = marker_path(parent_pid);
    thread::sleep(Duration::from_millis(1600));
    assert!(!marker.exists(), "descendant survived process-group kill");
    let _ = fs::remove_file(marker);
    Ok(())
}

#[cfg(unix)]
#[test]
fn escaped_descendant_cannot_hold_probe_readers_open() -> Result<(), Box<dyn std::error::Error>> {
    let runner = ProbeRunner::new(ProbeConfig {
        execution_enabled: true,
        timeout: Duration::from_millis(300),
        stdout_cap: 4096,
        stderr_cap: 4096,
    })?;
    let request = ProbeRequest::new(
        env::current_exe()?,
        helper_argv("probe_helper_escaped_parent"),
    )?;
    let started = std::time::Instant::now();
    let output = runner.run(&request)?;
    assert!(output.timed_out);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(output.stdout.truncated || output.stderr.truncated);

    let escaped = parse_escaped_pid(&String::from_utf8_lossy(&output.stdout.bytes))?;
    let _ = kill_process(escaped, Signal::KILL);
    Ok(())
}
