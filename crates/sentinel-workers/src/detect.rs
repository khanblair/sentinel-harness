use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::future::{join, join_all};
use sentinel_core::{Tier, ToolId, ToolInfo, ToolStatus};
use tokio::process::Command;

use crate::profile::{GENERIC_ID, LoginCheck, LoginKind, Profile};

/// Longest one scan may take. A tool still unanswered at this point is reported as failed, so a
/// busy computer cannot leave the scan spinning.
pub const SCAN_DEADLINE: Duration = Duration::from_secs(20);

/// Options that control detection.
pub struct DetectOptions {
    /// Search path for binaries. None means the real PATH.
    pub search_path: Option<OsString>,
    /// Home directory for tilde expansion in login checks.
    pub home: Option<PathBuf>,
    /// Extra tool names to look for on the path that have no profile.
    pub extra_names: Vec<String>,
    /// Timeout for version and login probes.
    pub probe_timeout: Duration,
}

impl Default for DetectOptions {
    fn default() -> Self {
        Self {
            search_path: None,
            home: None,
            extra_names: Vec::new(),
            probe_timeout: Duration::from_secs(5),
        }
    }
}

/// Run detection for all profiles and extra names at the same time.
/// The result keeps the profile order, then the extra names.
pub async fn detect_all(profiles: &[Profile], options: &DetectOptions) -> Vec<ToolInfo> {
    detect_all_within(profiles, options, SCAN_DEADLINE).await
}

async fn detect_all_within(
    profiles: &[Profile],
    options: &DetectOptions,
    deadline: Duration,
) -> Vec<ToolInfo> {
    let profile_jobs = profiles
        .iter()
        .filter(|profile| profile.id != GENERIC_ID)
        .map(|profile| detect_profile_within(profile, options, deadline));
    let extra_jobs = options
        .extra_names
        .iter()
        .filter(|name| !profiles.iter().any(|p| p.id == **name))
        .map(|name| detect_extra_within(name, options, deadline));

    let (profile_results, extra_results) = join(join_all(profile_jobs), join_all(extra_jobs)).await;
    profile_results
        .into_iter()
        .chain(extra_results.into_iter().flatten())
        .collect()
}

async fn detect_profile_within(
    profile: &Profile,
    options: &DetectOptions,
    deadline: Duration,
) -> ToolInfo {
    match tokio::time::timeout(deadline, detect_one(profile, options)).await {
        Ok(found) => found,
        Err(_) => ToolInfo {
            id: ToolId::new(&profile.id),
            name: profile.name.clone(),
            path: find_binary(&profile.binary, options)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default(),
            version: String::new(),
            adapter: profile.id.clone(),
            status: ToolStatus::Failed,
            tier: profile.tier,
            enabled: false,
        },
    }
}

async fn detect_extra_within(
    name: &str,
    options: &DetectOptions,
    deadline: Duration,
) -> Option<ToolInfo> {
    if let Ok(found) = tokio::time::timeout(deadline, detect_extra(name, options)).await {
        return found;
    }
    // Only a binary that was found can have a probe to time out.
    let binary_path = find_binary(name, options)?;
    Some(ToolInfo {
        id: ToolId::new(name),
        name: name.to_string(),
        path: binary_path.to_string_lossy().into_owned(),
        version: String::new(),
        adapter: GENERIC_ID.to_string(),
        status: ToolStatus::Failed,
        tier: Tier::Cheap,
        enabled: false,
    })
}

async fn detect_one(profile: &Profile, options: &DetectOptions) -> ToolInfo {
    let Some(binary_path) = find_binary(&profile.binary, options) else {
        return ToolInfo {
            id: ToolId::new(&profile.id),
            name: profile.name.clone(),
            path: String::new(),
            version: String::new(),
            adapter: profile.id.clone(),
            status: ToolStatus::Missing,
            tier: profile.tier,
            enabled: false,
        };
    };

    let version = probe_version(&binary_path, &profile.version_args, options.probe_timeout).await;

    let status = if version.is_empty() {
        ToolStatus::Failed
    } else {
        check_login(&profile.login, options).await
    };

    let final_status = match (status, profile.needs_template) {
        (ToolStatus::Ready, true) => ToolStatus::NeedsCommand,
        (s, _) => s,
    };

    ToolInfo {
        id: ToolId::new(&profile.id),
        name: profile.name.clone(),
        path: binary_path.to_string_lossy().into_owned(),
        version,
        adapter: profile.id.clone(),
        status: final_status,
        tier: profile.tier,
        enabled: false,
    }
}

async fn detect_extra(name: &str, options: &DetectOptions) -> Option<ToolInfo> {
    let binary_path = find_binary(name, options)?;
    let version = probe_version(
        &binary_path,
        &["--version".to_string()],
        options.probe_timeout,
    )
    .await;
    let status = if version.is_empty() {
        ToolStatus::Failed
    } else {
        ToolStatus::NeedsCommand
    };
    Some(ToolInfo {
        id: ToolId::new(name),
        name: name.to_string(),
        path: binary_path.to_string_lossy().into_owned(),
        version,
        adapter: GENERIC_ID.to_string(),
        status,
        tier: Tier::Cheap,
        enabled: false,
    })
}

fn find_binary(name: &str, options: &DetectOptions) -> Option<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // `which_in` looks nowhere when given no path, so "the real PATH" must be passed in.
    let search = options
        .search_path
        .clone()
        .or_else(|| std::env::var_os("PATH"));
    which::which_in(name, search, cwd).ok()
}

async fn probe_version(binary: &Path, args: &[String], timeout: Duration) -> String {
    let mut cmd = Command::new(binary);
    cmd.args(args);
    cmd.kill_on_drop(true);

    let Ok(Ok(output)) = tokio::time::timeout(timeout, cmd.output()).await else {
        return String::new();
    };

    if !output.status.success() {
        return String::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_version(&stdout)
}

fn extract_version(text: &str) -> String {
    for word in text.split_whitespace() {
        let candidate = word.trim_start_matches('v');
        let version_part: String = candidate
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if version_part.contains('.') {
            return version_part;
        }
    }
    "unknown".to_string()
}

async fn check_login(login: &LoginCheck, options: &DetectOptions) -> ToolStatus {
    match login.kind {
        LoginKind::None => ToolStatus::Ready,
        LoginKind::Dir => {
            let expanded = expand_tilde(&login.value, options);
            if expanded.exists() {
                ToolStatus::Ready
            } else {
                ToolStatus::NeedsLogin
            }
        }
        LoginKind::Env => {
            if std::env::var(&login.value).is_ok() {
                ToolStatus::Ready
            } else {
                ToolStatus::NeedsLogin
            }
        }
        LoginKind::Command => {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(&login.value);
            cmd.kill_on_drop(true);
            match tokio::time::timeout(options.probe_timeout, cmd.status()).await {
                Ok(Ok(status)) if status.success() => ToolStatus::Ready,
                _ => ToolStatus::NeedsLogin,
            }
        }
    }
}

fn expand_tilde(path: &str, options: &DetectOptions) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        let home = options
            .home
            .clone()
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .or_else(|| std::env::var("USERPROFILE").ok())
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| PathBuf::from("/"));
        home.join(stripped)
    } else {
        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn write_slow_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write fake binary");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("set permissions");
        path
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unanswered_tool_is_failed_when_scan_deadline_passes() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        write_slow_script(temp.path(), "slowtool", "sleep 5");
        let options = DetectOptions {
            search_path: Some(temp.path().as_os_str().to_owned()),
            home: Some(temp.path().to_path_buf()),
            extra_names: vec!["slowtool".to_string()],
            probe_timeout: Duration::from_secs(5),
        };

        let started = std::time::Instant::now();
        let results = detect_all_within(&[], &options, Duration::from_millis(200)).await;
        let elapsed = started.elapsed();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, ToolStatus::Failed);
        assert_eq!(results[0].version, "");
        assert_eq!(
            results[0].path,
            temp.path().join("slowtool").to_string_lossy()
        );
        assert!(elapsed < Duration::from_secs(3), "scan took {elapsed:?}");
    }

    #[test]
    fn extract_version_finds_first_token_with_digits_and_dots() {
        assert_eq!(extract_version("fake 1.2.3\n"), "1.2.3");
        assert_eq!(extract_version("v2.0.0-beta"), "2.0.0");
        assert_eq!(extract_version("version 3.14"), "3.14");
        assert_eq!(extract_version("no version here"), "unknown");
        assert_eq!(extract_version(""), "unknown");
    }

    #[test]
    fn expand_tilde_uses_home_dir() {
        let options = DetectOptions {
            home: Some(PathBuf::from("/home/user")),
            ..Default::default()
        };
        assert_eq!(
            expand_tilde("~/.claude", &options),
            PathBuf::from("/home/user/.claude")
        );
        assert_eq!(
            expand_tilde("/absolute/path", &options),
            PathBuf::from("/absolute/path")
        );
    }
}
