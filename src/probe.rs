use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[cfg(unix)]
use rustix::event::{PollFd, PollFlags, Timespec, poll};
#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};
#[cfg(unix)]
use std::os::fd::AsFd;
#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);
pub const DEFAULT_STDOUT_CAP: usize = 4 * 1024 * 1024;
pub const DEFAULT_STDERR_CAP: usize = 1024 * 1024;
pub const MAX_TIMEOUT: Duration = Duration::from_secs(30);
pub const MAX_STREAM_CAP: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ProbeConfig {
    pub execution_enabled: bool,
    pub timeout: Duration,
    pub stdout_cap: usize,
    pub stderr_cap: usize,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            execution_enabled: false,
            timeout: DEFAULT_TIMEOUT,
            stdout_cap: DEFAULT_STDOUT_CAP,
            stderr_cap: DEFAULT_STDERR_CAP,
        }
    }
}

impl ProbeConfig {
    pub fn validate(&self) -> Result<(), ProbeError> {
        if self.timeout.is_zero() || self.timeout > MAX_TIMEOUT {
            return Err(ProbeError::InvalidConfig(
                "timeout must be greater than zero and at most 30 seconds",
            ));
        }
        if self.stdout_cap == 0 || self.stdout_cap > MAX_STREAM_CAP {
            return Err(ProbeError::InvalidConfig(
                "stdout cap must be between 1 byte and 16 MiB",
            ));
        }
        if self.stderr_cap == 0 || self.stderr_cap > MAX_STREAM_CAP {
            return Err(ProbeError::InvalidConfig(
                "stderr cap must be between 1 byte and 16 MiB",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ProbeRequest {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
}

impl ProbeRequest {
    pub fn new(executable: PathBuf, argv: Vec<OsString>) -> Result<Self, ProbeError> {
        let request = Self { executable, argv };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), ProbeError> {
        if self.executable.as_os_str().is_empty() {
            return Err(ProbeError::InvalidRequest("executable must not be empty"));
        }
        if !self.executable.is_absolute() {
            return Err(ProbeError::InvalidRequest(
                "executable must be an absolute path",
            ));
        }
        if let Some(index) = self.argv.iter().position(|argument| argument.is_empty()) {
            return Err(ProbeError::EmptyArgument(index));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProbeRunner {
    config: ProbeConfig,
}

impl ProbeRunner {
    pub fn new(config: ProbeConfig) -> Result<Self, ProbeError> {
        config.validate()?;
        Ok(Self { config })
    }

    pub fn run(&self, request: &ProbeRequest) -> Result<ProbeOutput, ProbeError> {
        request.validate()?;
        if !self.config.execution_enabled {
            return Err(ProbeError::ExecutionDisabled);
        }

        let mut command = Command::new(&request.executable);
        command
            .args(&request.argv)
            .env_clear()
            .env("TERM", "dumb")
            .env("NO_COLOR", "1")
            // The executable request is already absolute; this fixed PATH only supports
            // interpreter-backed fixtures and bounded helper programs inside the probe.
            .env("PATH", "/usr/bin:/bin")
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("MANPAGER", "cat")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        command.process_group(0);

        let started = Instant::now();
        let mut child = command.spawn().map_err(ProbeError::Spawn)?;
        let stdout = child.stdout.take().ok_or_else(|| {
            let _ = terminate_and_reap(&mut child, None);
            ProbeError::MissingPipe("stdout")
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            let _ = terminate_and_reap(&mut child, None);
            ProbeError::MissingPipe("stderr")
        })?;
        let cancel_readers = Arc::new(AtomicBool::new(false));
        let stdout_reader =
            spawn_reader(stdout, self.config.stdout_cap, Arc::clone(&cancel_readers));
        let stderr_reader =
            spawn_reader(stderr, self.config.stderr_cap, Arc::clone(&cancel_readers));

        let mut status = None;
        let mut timed_out = false;
        let lifecycle = loop {
            if status.is_none() {
                match child.try_wait() {
                    Ok(observed) => status = observed,
                    Err(error) => {
                        break terminate_and_reap(&mut child, status)
                            .and(Err(ProbeError::Wait(error)));
                    }
                }
            }

            if stdout_reader.is_finished() && stderr_reader.is_finished() {
                if let Some(observed) = status {
                    break Ok(observed);
                }
            }

            if started.elapsed() >= self.config.timeout {
                if status.is_none() {
                    match child.try_wait() {
                        Ok(Some(observed)) => {
                            status = Some(observed);
                            continue;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            break terminate_and_reap(&mut child, status)
                                .and(Err(ProbeError::Wait(error)));
                        }
                    }
                }
                timed_out = true;
                break terminate_and_reap(&mut child, status);
            }
            thread::sleep(Duration::from_millis(5));
        };

        if timed_out || lifecycle.is_err() {
            cancel_readers.store(true, Ordering::Release);
        }
        let captures = join_captures(stdout_reader, stderr_reader);
        let status = lifecycle?;
        let (stdout, stderr) = captures?;

        Ok(ProbeOutput {
            executable: request.executable.clone(),
            argv: request.argv.clone(),
            duration: started.elapsed(),
            status: ProbeStatus::from_exit_status(status),
            timed_out,
            stdout,
            stderr,
        })
    }
}

#[derive(Debug)]
pub struct ProbeOutput {
    pub executable: PathBuf,
    pub argv: Vec<OsString>,
    pub duration: Duration,
    pub status: ProbeStatus,
    pub timed_out: bool,
    pub stdout: CapturedOutput,
    pub stderr: CapturedOutput,
}

#[derive(Debug)]
pub struct CapturedOutput {
    pub bytes: Vec<u8>,
    pub total_bytes: u64,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeStatus {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl ProbeStatus {
    fn from_exit_status(status: ExitStatus) -> Self {
        Self {
            code: status.code(),
            #[cfg(unix)]
            signal: status.signal(),
            #[cfg(not(unix))]
            signal: None,
        }
    }
}

#[derive(Debug)]
pub enum ProbeError {
    ExecutionDisabled,
    InvalidConfig(&'static str),
    InvalidRequest(&'static str),
    EmptyArgument(usize),
    Spawn(io::Error),
    Wait(io::Error),
    Terminate(io::Error),
    MissingPipe(&'static str),
    CaptureIo(io::Error),
    CaptureThreadPanic(&'static str),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExecutionDisabled => formatter.write_str("probe execution is disabled"),
            Self::InvalidConfig(message) => write!(formatter, "invalid probe config: {message}"),
            Self::InvalidRequest(message) => write!(formatter, "invalid probe request: {message}"),
            Self::EmptyArgument(index) => write!(formatter, "probe argv element {index} is empty"),
            Self::Spawn(error) => write!(formatter, "failed to spawn probe: {error}"),
            Self::Wait(error) => write!(formatter, "failed to wait for probe: {error}"),
            Self::Terminate(error) => write!(formatter, "failed to terminate probe: {error}"),
            Self::MissingPipe(stream) => write!(formatter, "probe {stream} pipe is unavailable"),
            Self::CaptureIo(error) => write!(formatter, "failed to capture probe output: {error}"),
            Self::CaptureThreadPanic(stream) => {
                write!(formatter, "probe {stream} capture thread panicked")
            }
        }
    }
}

impl Error for ProbeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Spawn(error)
            | Self::Wait(error)
            | Self::Terminate(error)
            | Self::CaptureIo(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(unix)]
fn spawn_reader<R>(
    reader: R,
    cap: usize,
    cancelled: Arc<AtomicBool>,
) -> JoinHandle<Result<CapturedOutput, io::Error>>
where
    R: AsFd + Read + Send + 'static,
{
    thread::spawn(move || capture(reader, cap, &cancelled))
}

#[cfg(not(unix))]
fn spawn_reader<R>(
    reader: R,
    cap: usize,
    _cancelled: Arc<AtomicBool>,
) -> JoinHandle<Result<CapturedOutput, io::Error>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || capture(reader, cap))
}

#[cfg(unix)]
fn capture<R: AsFd + Read>(
    mut reader: R,
    cap: usize,
    cancelled: &AtomicBool,
) -> Result<CapturedOutput, io::Error> {
    let mut bytes = Vec::with_capacity(cap);
    let mut total_bytes = 0_u64;
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];
    let poll_timeout = Timespec {
        tv_sec: 0,
        tv_nsec: 10_000_000,
    };
    loop {
        let (ready, events) = {
            let mut descriptors = [PollFd::new(
                &reader,
                PollFlags::IN | PollFlags::HUP | PollFlags::ERR,
            )];
            let ready = match poll(&mut descriptors, Some(&poll_timeout)) {
                Ok(ready) => ready,
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            (ready, descriptors[0].revents())
        };

        if ready == 0 {
            if cancelled.load(Ordering::Acquire) {
                truncated = true;
                break;
            }
            continue;
        }
        if events.contains(PollFlags::ERR) && !events.intersects(PollFlags::IN | PollFlags::HUP) {
            return Err(io::Error::other("probe output pipe reported an error"));
        }
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(count as u64);
        let retained = count.min(cap.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained != count;
        if cancelled.load(Ordering::Acquire) {
            truncated = true;
            break;
        }
    }
    Ok(CapturedOutput {
        bytes,
        total_bytes,
        truncated,
    })
}

#[cfg(not(unix))]
fn capture<R: Read>(mut reader: R, cap: usize) -> Result<CapturedOutput, io::Error> {
    let mut bytes = Vec::with_capacity(cap);
    let mut total_bytes = 0_u64;
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total_bytes = total_bytes.saturating_add(count as u64);
        let retained = count.min(cap.saturating_sub(bytes.len()));
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained != count;
    }
    Ok(CapturedOutput {
        bytes,
        total_bytes,
        truncated,
    })
}

fn join_captures(
    stdout: JoinHandle<Result<CapturedOutput, io::Error>>,
    stderr: JoinHandle<Result<CapturedOutput, io::Error>>,
) -> Result<(CapturedOutput, CapturedOutput), ProbeError> {
    let stdout = stdout
        .join()
        .map_err(|_| ProbeError::CaptureThreadPanic("stdout"))?
        .map_err(ProbeError::CaptureIo);
    let stderr = stderr
        .join()
        .map_err(|_| ProbeError::CaptureThreadPanic("stderr"))?
        .map_err(ProbeError::CaptureIo);
    Ok((stdout?, stderr?))
}

fn terminate_and_reap(
    child: &mut Child,
    observed: Option<ExitStatus>,
) -> Result<ExitStatus, ProbeError> {
    #[cfg(unix)]
    {
        let raw_pid = i32::try_from(child.id()).map_err(|_| {
            ProbeError::Terminate(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child process ID exceeds Unix pid range",
            ))
        })?;
        let pid = Pid::from_raw(raw_pid).ok_or_else(|| {
            ProbeError::Terminate(io::Error::new(
                io::ErrorKind::InvalidInput,
                "child process ID is zero",
            ))
        })?;
        if let Err(error) = kill_process_group(pid, Signal::KILL) {
            let error = io::Error::from(error);
            if error.raw_os_error() != Some(3) || observed.is_none() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ProbeError::Terminate(error));
            }
        }
    }

    let _ = child.kill();
    match observed {
        Some(status) => Ok(status),
        None => child.wait().map_err(ProbeError::Wait),
    }
}
