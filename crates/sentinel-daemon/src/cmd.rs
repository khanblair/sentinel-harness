//! Run external programs behind one trait.
//!
//! The daemon reaches Tailscale, Rust, and the doctor tools by shelling out. The trait keeps
//! every caller free of a real spawn, so tests feed a `FakeRunner` with canned answers.

use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use futures::future::BoxFuture;
use tokio::process::Command;

/// What one program run printed. `status` is `None` when the program never started or
/// was killed after the timeout.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CmdOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CmdOutput {
    pub fn ok(stdout: &str) -> Self {
        Self {
            status: Some(0),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        }
    }

    pub fn failure(status: i32, stderr: &str) -> Self {
        Self {
            status: Some(status),
            stdout: String::new(),
            stderr: stderr.to_owned(),
        }
    }

    pub fn never_started() -> Self {
        Self::default()
    }
}

/// The one seam between the daemon and the programs it calls. Every caller takes
/// `&dyn CommandRunner`, so tests can hand over a fake and the binary a real runner.
pub trait CommandRunner: Send + Sync {
    fn run<'a>(&'a self, program: &'a str, args: &'a [&'a str]) -> BoxFuture<'a, CmdOutput>;
}

/// Runs the real program. A run gets 10 seconds, then the child process is killed.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRunner;

const RUN_TIMEOUT_SECONDS: u64 = 10;

impl CommandRunner for SystemRunner {
    fn run<'a>(&'a self, program: &'a str, args: &'a [&'a str]) -> BoxFuture<'a, CmdOutput> {
        Box::pin(async move {
            let child = Command::new(program)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn();
            let Ok(child) = child else {
                return CmdOutput::never_started();
            };
            let wait = child.wait_with_output();
            match tokio::time::timeout(Duration::from_secs(RUN_TIMEOUT_SECONDS), wait).await {
                // The timeout drops the child, and `kill_on_drop` stops it there.
                Err(_) => CmdOutput {
                    status: None,
                    stdout: String::new(),
                    stderr: format!(
                        "{program} did not finish within {RUN_TIMEOUT_SECONDS} seconds."
                    ),
                },
                Ok(Ok(output)) => CmdOutput {
                    status: output.status.code(),
                    stdout: as_text(&output.stdout),
                    stderr: as_text(&output.stderr),
                },
                Ok(Err(_)) => CmdOutput::never_started(),
            }
        })
    }
}

fn as_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Canned answers for tests. A key is the command line joined by spaces, such as
/// `tailscale status --json`. An unknown key reports a program that never started.
#[derive(Debug, Clone, Default)]
pub struct FakeRunner {
    outputs: HashMap<String, CmdOutput>,
}

impl FakeRunner {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, command: &str, output: CmdOutput) -> Self {
        self.outputs.insert(command.to_owned(), output);
        self
    }

    fn answer(&self, key: &str) -> CmdOutput {
        self.outputs
            .get(key)
            .cloned()
            .unwrap_or_else(CmdOutput::never_started)
    }
}

impl CommandRunner for FakeRunner {
    fn run<'a>(&'a self, program: &'a str, args: &'a [&'a str]) -> BoxFuture<'a, CmdOutput> {
        Box::pin(async move {
            let key = joined_command(program, args);
            self.answer(&key)
        })
    }
}

fn joined_command(program: &str, args: &[&str]) -> String {
    let mut key = program.to_owned();
    for arg in args {
        key.push(' ');
        key.push_str(arg);
    }
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_runner_answers_the_whole_command_line() {
        let runner = FakeRunner::new().with("tailscale status", CmdOutput::ok("ok"));
        let output = runner.run("tailscale", &["status"]).await;
        assert_eq!(output.status, Some(0));
        assert_eq!(output.stdout, "ok");
    }

    #[tokio::test]
    async fn fake_runner_reports_a_never_started_program_for_unknown_keys() {
        let runner = FakeRunner::new().with("trunk --version", CmdOutput::ok("trunk 0.21"));
        let output = runner.run("trunk", &["--version", "--extra"]).await;
        assert_eq!(output, CmdOutput::never_started());
    }

    #[tokio::test]
    async fn fake_runner_keeps_a_failure_canned() {
        let runner = FakeRunner::new().with("git --version", CmdOutput::failure(1, "boom\n"));
        let output = runner.run("git", &["--version"]).await;
        assert_eq!(output.stderr, "boom\n");
    }
}
