use std::fs;

use serde_json::json;
use tempfile::TempDir;

use super::support::{call_tool, exchange, is_error, tool_text};

/// A repository folder with the given scope, as the agent CLI would set it up.
fn settings(repo: &TempDir, scope: &str) -> Vec<(&'static str, String)> {
    vec![
        ("SENTINEL_WORKDIR", repo.path().display().to_string()),
        ("SENTINEL_SCOPE", scope.to_owned()),
    ]
}

/// Send one `write_file` call and return its reply.
fn write(repo: &TempDir, scope: &str, path: &str, content: &str) -> serde_json::Value {
    let env = settings(repo, scope);
    let env_refs: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let arguments = json!({"path": path, "content": content});
    exchange(&env_refs, &[call_tool(1, "write_file", arguments)])
        .into_iter()
        .next()
        .expect("one reply")
}

#[test]
fn write_file_inside_the_scope_writes_the_file() {
    let repo = TempDir::new().expect("temp repo");
    fs::create_dir(repo.path().join("docs")).expect("docs folder");
    let reply = write(&repo, "src/lib.rs\ndocs", "src/lib.rs", "fn main() {}");
    assert!(!is_error(&reply), "{}", tool_text(&reply));
    assert_eq!(
        fs::read_to_string(repo.path().join("src/lib.rs")).expect("file written"),
        "fn main() {}"
    );
    let folder_reply = write(&repo, "src/lib.rs\ndocs", "docs/notes.md", "hello");
    assert!(!is_error(&folder_reply), "{}", tool_text(&folder_reply));
    assert!(repo.path().join("docs/notes.md").is_file());
}

#[test]
fn write_file_outside_the_scope_is_refused_and_writes_nothing() {
    let repo = TempDir::new().expect("temp repo");
    let reply = write(&repo, "src/lib.rs", "other.txt", "nope");
    assert!(is_error(&reply));
    assert!(tool_text(&reply).contains("not reserved"));
    assert!(!repo.path().join("other.txt").exists());
}

#[test]
fn write_file_refuses_traversal_and_absolute_paths() {
    let repo = TempDir::new().expect("temp repo");
    let outside = repo
        .path()
        .parent()
        .expect("temp parent")
        .join("escape.txt");
    let absolute = outside.display().to_string();
    for path in ["../escape.txt", absolute.as_str()] {
        let reply = write(&repo, "src", path, "nope");
        assert!(is_error(&reply), "{path} must be refused");
    }
    assert!(!outside.exists());
}

#[cfg(unix)]
#[test]
fn write_file_refuses_a_symlink_that_points_outside_the_repo() {
    let repo = TempDir::new().expect("temp repo");
    let elsewhere = TempDir::new().expect("temp folder outside the repo");
    std::os::unix::fs::symlink(elsewhere.path(), repo.path().join("link")).expect("symlink");
    let reply = write(&repo, "link", "link/owned.txt", "nope");
    assert!(is_error(&reply));
    assert!(!elsewhere.path().join("owned.txt").exists());
}
