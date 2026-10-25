//! Finds the agent CLIs when the Mac app starts from Finder, Spotlight, or the Dock. Those
//! launches get the system PATH, not the PATH of the user's shell, so the agents and the
//! scripts they run (many start with `#!/usr/bin/env node`) cannot be found.

use std::ffi::{OsStr, OsString};
#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::time::{Duration, Instant};

/// Set in the restarted process so a second round never starts.
const GUARD_VAR: &str = "SENTINEL_PATH_FIXED";
const MARKER: &str = "__SENTINEL_PATH__";
#[cfg(unix)]
const LOGIN_SHELL_SCRIPT: &str = r#"printf "\n__SENTINEL_PATH__%s\n" "$PATH""#;
#[cfg(unix)]
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(unix)]
const POLL_INTERVAL: Duration = Duration::from_millis(50);
const SYSTEM_FOLDERS: [&str; 3] = ["/opt/homebrew/bin", "/opt/homebrew/sbin", "/usr/local/bin"];
const HOME_FOLDERS: [&str; 6] = [
    ".local/bin",
    ".cargo/bin",
    ".bun/bin",
    ".npm-global/bin",
    "go/bin",
    ".deno/bin",
];

/// Merges the current PATH, the login shell PATH, and the usual install folders. Returns `None`
/// when the merged PATH equals the current one.
pub(crate) fn repaired_path(
    current: &OsStr,
    home: &Path,
    shell_path: Option<&OsStr>,
) -> Option<OsString> {
    let shell_entries = shell_path.into_iter().flat_map(std::env::split_paths);
    let common = common_folders(home)
        .into_iter()
        .filter(|folder| folder.is_dir());
    let candidates = std::env::split_paths(current)
        .chain(shell_entries)
        .chain(common);
    let mut entries: Vec<PathBuf> = Vec::new();
    for entry in candidates {
        if entry.is_absolute() && !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    let joined = std::env::join_paths(&entries).ok()?;
    (joined.as_os_str() != current).then_some(joined)
}

fn common_folders(home: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = SYSTEM_FOLDERS.into_iter().map(PathBuf::from).collect();
    folders.extend(HOME_FOLDERS.into_iter().map(|folder| home.join(folder)));
    folders
}

/// Reads the PATH that the login shell prints after the marker. Interactive shells print noise
/// before it, so only the text after the last marker counts.
pub(crate) fn parse_login_output(stdout: &str) -> Option<OsString> {
    let (_, after_marker) = stdout.rsplit_once(MARKER)?;
    let line = after_marker.lines().next()?;
    let all_absolute = line
        .split(':')
        .filter(|entry| !entry.is_empty())
        .all(|entry| entry.starts_with('/'));
    (!line.is_empty() && all_absolute).then_some(OsString::from(line))
}

/// Asks the user's login shell for its PATH. Fish and nushell are skipped: their PATH syntax
/// differs from the POSIX shells this reads.
#[cfg(unix)]
fn login_shell_path() -> Option<OsString> {
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/zsh"));
    let name = Path::new(&shell).file_name()?;
    if name == "fish" || name == "nu" {
        return None;
    }
    let mut child = Command::new(&shell)
        .args(["-ilc", LOGIN_SHELL_SCRIPT])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    if !wait_for_exit(&mut child) {
        return None;
    }
    let mut stdout = String::new();
    child.stdout.take()?.read_to_string(&mut stdout).ok()?;
    parse_login_output(&stdout)
}

/// Returns true when the child exits in time. A child that runs longer is killed.
#[cfg(unix)]
fn wait_for_exit(child: &mut Child) -> bool {
    let deadline = Instant::now() + LOGIN_SHELL_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Starts this program again with the repaired PATH. `exec` replaces the process and keeps its
/// id. It returns only when the restart fails, and then the old PATH stays in use.
#[cfg(unix)]
pub(crate) fn ensure_login_path() {
    if std::env::var_os(GUARD_VAR).is_some() {
        return;
    }
    let current = std::env::var_os("PATH").unwrap_or_default();
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let Some(path) = repaired_path(&current, &home, login_shell_path().as_deref()) else {
        return;
    };
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let error = Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env("PATH", &path)
        .env(GUARD_VAR, "1")
        .exec();
    eprintln!("warning: could not restart with the repaired PATH: {error}");
}

#[cfg(not(unix))]
pub(crate) fn ensure_login_path() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repaired_path_keeps_current_order_and_adds_shell_and_home_folders() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".local/bin")).unwrap();
        let current = OsStr::new("/usr/bin:/bin::relative/bin:/usr/bin");
        let shell = OsStr::new("/bin:/Users/me/custom:shell-relative");

        let path = repaired_path(current, home.path(), Some(shell)).unwrap();
        let entries: Vec<PathBuf> = std::env::split_paths(&path).collect();

        let expected_start = [
            PathBuf::from("/usr/bin"),
            PathBuf::from("/bin"),
            PathBuf::from("/Users/me/custom"),
        ];
        assert_eq!(entries[..3], expected_start);
        assert!(entries.contains(&home.path().join(".local/bin")));
        assert!(!entries.contains(&home.path().join(".cargo/bin")));
        assert!(entries.iter().all(|entry| entry.is_absolute()));
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.as_os_str() == "/usr/bin")
                .count(),
            1
        );
    }

    #[test]
    fn repaired_path_returns_none_when_every_folder_is_already_listed() {
        let home = tempfile::tempdir().unwrap();
        let existing = common_folders(home.path())
            .into_iter()
            .filter(|folder| folder.is_dir());
        let base = ["/usr/bin", "/bin"].into_iter().map(PathBuf::from);
        let current = std::env::join_paths(base.chain(existing)).unwrap();

        assert_eq!(repaired_path(&current, home.path(), None), None);
    }

    #[test]
    fn parse_login_output_takes_the_text_after_the_last_marker() {
        let output = "Welcome, noise\n__SENTINEL_PATH__/old/bin\n\n__SENTINEL_PATH__/usr/bin:/opt/homebrew/bin\n";

        assert_eq!(
            parse_login_output(output),
            Some(OsString::from("/usr/bin:/opt/homebrew/bin"))
        );
    }

    #[test]
    fn parse_login_output_rejects_a_path_with_a_relative_entry() {
        assert_eq!(parse_login_output("__SENTINEL_PATH__/usr/bin:bin\n"), None);
    }

    #[test]
    fn parse_login_output_rejects_output_without_a_marker() {
        assert_eq!(parse_login_output("/usr/bin:/bin\n"), None);
    }
}
