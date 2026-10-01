use crate::schema::{Confidence, HelpSource};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbePlan {
    pub argv: Vec<String>,
    pub parser: &'static str,
    pub source: HelpSource,
    pub confidence: Confidence,
    pub command_path: Vec<String>,
}

pub fn plan(command: &[String]) -> Option<ProbePlan> {
    let executable = command
        .first()
        .and_then(|value| Path::new(value).file_name())
        .and_then(|value| value.to_str())?;
    let mut command_path = command.to_vec();
    command_path[0] = executable.to_owned();
    match (
        executable,
        command.get(1).map(String::as_str),
        command.len(),
    ) {
        ("git", Some("commit" | "remote"), 2) => Some(ProbePlan {
            argv: vec![command[1].clone(), "-h".into()],
            parser: "git-help-v1",
            source: HelpSource::FrameworkHelp,
            confidence: Confidence::High,
            command_path: command_path.clone(),
        }),
        ("ffmpeg", None, 1) => Some(ProbePlan {
            argv: vec!["-h".into(), "long".into()],
            parser: "ffmpeg-help-v1",
            source: HelpSource::FrameworkHelp,
            confidence: Confidence::High,
            command_path,
        }),
        ("docker", Some("run"), 2) => Some(ProbePlan {
            argv: vec![command[1].clone(), "--help".into()],
            parser: "docker-help-v1",
            source: HelpSource::FrameworkHelp,
            confidence: Confidence::High,
            command_path,
        }),
        ("kubectl", Some("get"), 2) => Some(ProbePlan {
            argv: vec![command[1].clone(), "--help".into()],
            parser: "kubectl-help-v1",
            source: HelpSource::FrameworkHelp,
            confidence: Confidence::High,
            command_path,
        }),
        ("curl", None, 1) => Some(ProbePlan {
            argv: vec!["--help".into(), "all".into()],
            parser: "curl-help-v1",
            source: HelpSource::FrameworkHelp,
            confidence: Confidence::High,
            command_path,
        }),
        _ => None,
    }
}
