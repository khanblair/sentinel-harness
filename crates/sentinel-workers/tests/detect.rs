use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sentinel_core::{Tier, ToolId, ToolStatus};
use sentinel_workers::{
    detect::{DetectOptions, detect_all},
    profile::{GENERIC_ID, LoginCheck, LoginKind, Profile, PromptVia},
};
use tempfile::TempDir;
use tokio::fs;

async fn write_fake_binary(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    let script = format!("#!/bin/sh\n{content}\n");
    fs::write(&path, script).await.expect("write fake binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)
            .await
            .expect("metadata fake binary")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)
            .await
            .expect("set permissions");
    }
    path
}

fn make_profiles() -> Vec<Profile> {
    vec![
        base_profile("opencode", "OpenCode", true, LoginKind::None, "", false),
        base_profile(
            "testlogin",
            "TestLogin",
            false,
            LoginKind::Dir,
            "~/.testlogin",
            false,
        ),
        base_profile(
            "needstemplate",
            "NeedsTemplate",
            false,
            LoginKind::None,
            "",
            true,
        ),
        generic_profile(),
    ]
}

#[allow(clippy::too_many_arguments)]
fn base_profile(
    id: &str,
    name: &str,
    verified: bool,
    login_kind: LoginKind,
    login_value: &str,
    needs_template: bool,
) -> Profile {
    Profile {
        id: id.to_string(),
        name: name.to_string(),
        binary: id.to_string(),
        version_args: vec!["--version".to_string()],
        tier: Tier::Cheap,
        prompt_via: PromptVia::Arg,
        args: vec!["{prompt}".to_string()],
        default_model: if id == "opencode" {
            "opencode/model".to_string()
        } else {
            String::new()
        },
        verified,
        needs_template,
        cost_prefix: String::new(),
        login: LoginCheck {
            kind: login_kind,
            value: login_value.to_string(),
        },
        env: BTreeMap::default(),
    }
}

fn generic_profile() -> Profile {
    Profile {
        id: GENERIC_ID.to_string(),
        name: "Generic".to_string(),
        binary: String::new(),
        version_args: vec!["--version".to_string()],
        tier: Tier::Cheap,
        prompt_via: PromptVia::Arg,
        args: vec!["{prompt}".to_string()],
        default_model: String::new(),
        verified: false,
        needs_template: true,
        cost_prefix: String::new(),
        login: LoginCheck {
            kind: LoginKind::None,
            value: String::new(),
        },
        env: BTreeMap::default(),
    }
}

fn make_options(bin_dir: &Path, home: &Path, extra: Vec<String>) -> DetectOptions {
    DetectOptions {
        search_path: Some(OsString::from(bin_dir)),
        home: Some(home.to_path_buf()),
        extra_names: extra,
        probe_timeout: Duration::from_millis(500),
    }
}

#[tokio::test]
async fn found_binary_with_version_becomes_ready() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "opencode", "echo \"fake 1.2.3\"").await;

    let options = make_options(&bin_dir, temp.path(), vec![]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("opencode"))
        .expect("opencode found");

    assert_eq!(found.status, ToolStatus::Ready);
    assert_eq!(found.version, "1.2.3");
    assert_eq!(found.path, bin_dir.join("opencode").to_string_lossy());
    assert_eq!(found.adapter, "opencode");
    assert_eq!(found.tier, Tier::Cheap);
    assert!(!found.enabled);
}

#[tokio::test]
async fn missing_binary_is_missing() {
    let temp = TempDir::new().expect("temp dir");
    let options = make_options(temp.path(), temp.path(), vec![]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("opencode"))
        .expect("opencode found");

    assert_eq!(found.status, ToolStatus::Missing);
    assert_eq!(found.path, "");
    assert_eq!(found.version, "");
}

#[tokio::test]
async fn dir_login_missing_folder_is_needs_login() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "testlogin", "echo \"fake 1.0.0\"").await;

    let options = make_options(&bin_dir, temp.path(), vec![]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("testlogin"))
        .expect("testlogin found");

    assert_eq!(found.status, ToolStatus::NeedsLogin);
}

#[tokio::test]
async fn dir_login_present_folder_is_ready() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "testlogin", "echo \"fake 1.0.0\"").await;
    fs::create_dir(temp.path().join(".testlogin"))
        .await
        .expect("create dir");

    let options = make_options(&bin_dir, temp.path(), vec![]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("testlogin"))
        .expect("testlogin found");

    assert_eq!(found.status, ToolStatus::Ready);
}

#[tokio::test]
async fn version_probe_failure_is_failed() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "opencode", "exit 1").await;

    let options = make_options(&bin_dir, temp.path(), vec![]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("opencode"))
        .expect("opencode found");

    assert_eq!(found.status, ToolStatus::Failed);
}

#[tokio::test]
async fn version_probe_timeout_is_failed() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "opencode", "sleep 1").await;

    let options = DetectOptions {
        search_path: Some(OsString::from(&bin_dir)),
        home: Some(temp.path().to_path_buf()),
        extra_names: vec![],
        probe_timeout: Duration::from_millis(200),
    };

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("opencode"))
        .expect("opencode found");

    assert_eq!(found.status, ToolStatus::Failed);
}

#[tokio::test]
async fn extra_name_on_path_becomes_generic_needs_command() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "mytool", "echo \"mytool 2.0.0\"").await;

    let options = make_options(&bin_dir, temp.path(), vec!["mytool".to_string()]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("mytool"))
        .expect("mytool found");

    assert_eq!(found.adapter, GENERIC_ID);
    assert_eq!(found.status, ToolStatus::NeedsCommand);
    assert_eq!(found.version, "2.0.0");
    assert_eq!(found.tier, Tier::Cheap);
}

#[tokio::test]
async fn needs_template_profile_found_is_needs_command() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "needstemplate", "echo \"fake 1.0.0\"").await;

    let options = make_options(&bin_dir, temp.path(), vec![]);

    let results = detect_all(&make_profiles(), &options).await;
    let found = results
        .iter()
        .find(|i| i.id == ToolId::new("needstemplate"))
        .expect("needstemplate found");

    assert_eq!(found.status, ToolStatus::NeedsCommand);
    assert_eq!(found.adapter, "needstemplate");
}

#[tokio::test]
async fn result_order_is_profile_order_then_extras() {
    let temp = TempDir::new().expect("temp dir");
    let bin_dir = temp.path().to_path_buf();
    write_fake_binary(temp.path(), "opencode", "echo \"1.0\"").await;
    write_fake_binary(temp.path(), "testlogin", "echo \"1.0\"").await;
    write_fake_binary(temp.path(), "needstemplate", "echo \"1.0\"").await;
    write_fake_binary(temp.path(), "myextra", "echo \"1.0\"").await;

    let options = make_options(&bin_dir, temp.path(), vec!["myextra".to_string()]);

    let results = detect_all(&make_profiles(), &options).await;

    let ids: Vec<&str> = results.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["opencode", "testlogin", "needstemplate", "myextra"]);
}

#[tokio::test]
async fn with_no_search_path_the_real_path_is_used() {
    // `sh` is on every PATH where these tests run. Before the fix, no path meant no search.
    let mut profile = base_profile("sh", "Shell", true, LoginKind::None, "", false);
    profile.binary = "sh".to_owned();
    profile.version_args = vec!["-c".to_owned(), "echo shell 1.0.0".to_owned()];
    let options = DetectOptions {
        search_path: None,
        home: None,
        extra_names: Vec::new(),
        probe_timeout: Duration::from_secs(5),
    };

    let found = detect_all(&[profile], &options).await;

    assert_eq!(found[0].status, ToolStatus::Ready);
    assert_ne!(found[0].path, "");
}
