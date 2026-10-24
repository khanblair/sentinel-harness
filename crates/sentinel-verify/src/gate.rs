//! Test gate: runs the full suite at a barrier.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::{Result, VerifyError};

/// Configuration for a test suite run.
#[derive(Debug, Clone)]
pub struct SuiteConfig {
    /// Command to run (program plus args), e.g., ["cargo", "test"].
    pub command: Vec<String>,
    /// Timeout for the entire suite.
    pub timeout: Duration,
    /// Directory where log files are written.
    pub log_dir: PathBuf,
}

/// Result of a test suite run.
#[derive(Debug, Clone)]
pub struct SuiteResult {
    /// Whether the suite passed.
    pub passed: bool,
    /// Exit code of the process, if it exited normally.
    pub exit_code: Option<i32>,
    /// Whether the run timed out.
    pub timed_out: bool,
    /// Path to the log file.
    pub log_ref: PathBuf,
    /// Names of failing tests parsed from the output.
    pub failing_tests: Vec<String>,
    /// Last 30 lines of output (tail).
    pub tail: Vec<String>,
}

/// Runs a test suite command and returns the result.
///
/// The command is run in `root` with stdout and stderr combined into a log file
/// at `config.log_dir/<label>.log`. The function never reads the entire log into
/// memory; it keeps only the last 30 lines and parses failing test names from
/// lines matching `test NAME ... FAILED` or `FAILED: NAME`.
pub async fn run_suite(root: &Path, config: &SuiteConfig, label: &str) -> Result<SuiteResult> {
    if config.command.is_empty() {
        return Err(VerifyError::Gate("empty command".to_string()));
    }

    // Ensure log directory exists
    tokio::fs::create_dir_all(&config.log_dir).await?;

    let log_path = config.log_dir.join(format!("{label}.log"));
    let log_file = tokio::fs::File::create(&log_path).await?;
    let log_file = std::sync::Arc::new(tokio::sync::Mutex::new(log_file));

    let mut cmd = Command::new(&config.command[0]);
    cmd.args(&config.command[1..])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let log_file_clone = log_file.clone();
    let tail_lines: std::sync::Arc<tokio::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let failing_tests: std::sync::Arc<tokio::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));

    // Task to read stdout
    let tail_lines_stdout = tail_lines.clone();
    let failing_tests_stdout = failing_tests.clone();
    let log_file_stdout = log_file_clone.clone();
    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            write_log(&log_file_stdout, &line).await;
            update_tail(&tail_lines_stdout, line.clone()).await;
            parse_failing_test(&line, &failing_tests_stdout).await;
        }
    });

    // Task to read stderr
    let tail_lines_stderr = tail_lines.clone();
    let failing_tests_stderr = failing_tests.clone();
    let log_file_stderr = log_file_clone.clone();
    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            write_log(&log_file_stderr, &line).await;
            update_tail(&tail_lines_stderr, line.clone()).await;
            parse_failing_test(&line, &failing_tests_stderr).await;
        }
    });

    // Wait for the process with timeout
    let (timed_out, exit_code) = match timeout(config.timeout, child.wait()).await {
        Ok(Ok(status)) => {
            stdout_task.await.ok();
            stderr_task.await.ok();
            (false, status.code())
        }
        Ok(Err(e)) => {
            stdout_task.await.ok();
            stderr_task.await.ok();
            return Err(VerifyError::Io(e));
        }
        Err(_) => {
            // Timeout - kill the child
            let _ = child.kill().await;
            stdout_task.await.ok();
            stderr_task.await.ok();
            (true, None)
        }
    };

    let tail = tail_lines.lock().await.clone();
    let failing_tests = failing_tests.lock().await.clone();

    let passed = !timed_out && exit_code == Some(0) && failing_tests.is_empty();

    Ok(SuiteResult {
        passed,
        exit_code,
        timed_out,
        log_ref: log_path,
        failing_tests,
        tail,
    })
}

async fn write_log(log_file: &std::sync::Arc<tokio::sync::Mutex<tokio::fs::File>>, line: &str) {
    use tokio::io::AsyncWriteExt;
    let mut file = log_file.lock().await;
    let _ = file.write_all(line.as_bytes()).await;
    let _ = file.write_all(b"\n").await;
}

async fn update_tail(tail_lines: &std::sync::Arc<tokio::sync::Mutex<Vec<String>>>, line: String) {
    let mut tail = tail_lines.lock().await;
    tail.push(line);
    if tail.len() > 30 {
        let drain_count = tail.len() - 30;
        tail.drain(0..drain_count);
    }
}

async fn parse_failing_test(
    line: &str,
    failing_tests: &std::sync::Arc<tokio::sync::Mutex<Vec<String>>>,
) {
    // Match: test NAME ... FAILED
    if let Some(name) = parse_rust_test_failed(line) {
        let mut tests = failing_tests.lock().await;
        if !tests.contains(&name) {
            tests.push(name);
        }
        return;
    }

    // Match: FAILED: NAME
    if let Some(name) = parse_failed_colon(line) {
        let mut tests = failing_tests.lock().await;
        if !tests.contains(&name) {
            tests.push(name);
        }
    }
}

fn parse_rust_test_failed(line: &str) -> Option<String> {
    // Pattern: "test NAME ... FAILED"
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("test ")
        && let Some(name_end) = rest.find(" ... FAILED")
    {
        let name = rest[..name_end].trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

fn parse_failed_colon(line: &str) -> Option<String> {
    // Pattern: "FAILED: NAME"
    let line = line.trim();
    if let Some(name) = line.strip_prefix("FAILED:") {
        let name = name.trim();
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test]
    async fn gate_passes_and_writes_log() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        let config = SuiteConfig {
            command: vec!["sh".into(), "-c".into(), "echo hi; exit 0".into()],
            timeout: Duration::from_secs(10),
            log_dir: log_dir.clone(),
        };

        let result = run_suite(root, &config, "test-pass").await.unwrap();
        assert!(result.passed);
        assert!(!result.timed_out);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.log_ref.exists());
        assert!(result.log_ref.to_string_lossy().ends_with("test-pass.log"));
        assert!(result.failing_tests.is_empty());
        assert!(!result.tail.is_empty());
    }

    #[tokio::test]
    async fn gate_fails_on_exit_1() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        let config = SuiteConfig {
            command: vec!["sh".into(), "-c".into(), "echo fail; exit 1".into()],
            timeout: Duration::from_secs(10),
            log_dir: log_dir.clone(),
        };

        let result = run_suite(root, &config, "test-fail").await.unwrap();
        assert!(!result.passed);
        assert!(!result.timed_out);
        assert_eq!(result.exit_code, Some(1));
    }

    #[tokio::test]
    async fn gate_parses_failing_tests() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        // Script that outputs Rust test failure format
        let script = r#"
echo "test a::b ... ok"
echo "test x::y ... FAILED"
echo "test foo::bar ... FAILED"
echo "test a::b ... ok"
"#;

        let config = SuiteConfig {
            command: vec!["sh".into(), "-c".into(), script.into()],
            timeout: Duration::from_secs(10),
            log_dir: log_dir.clone(),
        };

        let result = run_suite(root, &config, "test-parse").await.unwrap();
        assert!(!result.passed);
        assert_eq!(result.failing_tests.len(), 2);
        assert!(result.failing_tests.contains(&"x::y".to_string()));
        assert!(result.failing_tests.contains(&"foo::bar".to_string()));
    }

    #[tokio::test]
    async fn gate_parses_failed_colon_format() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        let script = r#"
echo "FAILED: test_something"
echo "FAILED: another_test"
"#;

        let config = SuiteConfig {
            command: vec!["sh".into(), "-c".into(), script.into()],
            timeout: Duration::from_secs(10),
            log_dir: log_dir.clone(),
        };

        let result = run_suite(root, &config, "test-colon").await.unwrap();
        assert_eq!(result.failing_tests.len(), 2);
        assert!(result.failing_tests.contains(&"test_something".to_string()));
        assert!(result.failing_tests.contains(&"another_test".to_string()));
    }

    #[tokio::test]
    async fn gate_tail_is_bounded() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        // Generate many lines
        let script = (0..2000)
            .map(|i| format!("echo line{}", i))
            .collect::<Vec<_>>()
            .join("; ");

        let config = SuiteConfig {
            command: vec!["sh".into(), "-c".into(), script],
            timeout: Duration::from_secs(30),
            log_dir: log_dir.clone(),
        };

        let result = run_suite(root, &config, "test-tail").await.unwrap();
        assert_eq!(result.tail.len(), 30);
    }

    #[tokio::test]
    async fn gate_timeout_kills_child() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        // Sleep longer than timeout
        let config = SuiteConfig {
            command: vec!["sh".into(), "-c".into(), "sleep 10".into()],
            timeout: Duration::from_millis(100),
            log_dir: log_dir.clone(),
        };

        let result = run_suite(root, &config, "test-timeout").await.unwrap();
        assert!(result.timed_out);
        assert!(!result.passed);
        assert_eq!(result.exit_code, None);
    }

    #[tokio::test]
    async fn gate_empty_command_is_error() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let log_dir = root.join("logs");

        let config = SuiteConfig {
            command: vec![],
            timeout: Duration::from_secs(10),
            log_dir,
        };

        let result = run_suite(root, &config, "test-empty").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            VerifyError::Gate(msg) => assert_eq!(msg, "empty command"),
            _ => panic!("expected Gate error"),
        }
    }
}
