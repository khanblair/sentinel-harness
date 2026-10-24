//! Tests for the contrast check.

#![allow(clippy::unwrap_used, reason = "tests fail by panicking")]

use std::fs;
use std::path::Path;

use tempfile::tempdir;

use super::color::{contrast, luminance};
use super::run;
use crate::finding::CheckResult;

// ---- Contrast math tests ----

#[test]
fn contrast_black_on_white_is_21() {
    let ratio = contrast("#000000", "#FFFFFF");
    assert!((ratio - 21.0).abs() < 0.01, "got {ratio}");
}

#[test]
fn contrast_white_on_white_is_1() {
    let ratio = contrast("#FFFFFF", "#FFFFFF");
    assert!((ratio - 1.0).abs() < 0.01, "got {ratio}");
}

#[test]
fn luminance_known_values() {
    // White
    assert!((luminance("#FFFFFF").unwrap() - 1.0).abs() < 0.001);
    // Black
    assert!((luminance("#000000").unwrap() - 0.0).abs() < 0.001);
    // Mid-gray #808080
    let l = luminance("#808080").unwrap();
    assert!(l > 0.2 && l < 0.3, "got {l}");
}

// ---- Fixture-based tests ----
// Fixtures are loaded from files to avoid `#RRGGBB` being parsed as raw identifiers.

fn fixture_dir() -> std::path::PathBuf {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().join("fixtures/contrast")
}

fn run_on_fixture_file(name: &str) -> CheckResult {
    let dir = tempdir().unwrap();
    let fixture_path = fixture_dir().join(name);
    let fixture = fs::read_to_string(&fixture_path).unwrap();

    let prefs = dir.path().join("prefs.rs");
    fs::write(&prefs, fixture).unwrap();

    // We need to mimic the file structure: root/crates/sentinel-ui/src/prefs.rs
    let root = dir.path().join("root");
    let ui_src = root.join("crates/sentinel-ui/src");
    fs::create_dir_all(&ui_src).unwrap();
    fs::copy(&prefs, ui_src.join("prefs.rs")).unwrap();

    run(&root)
}

#[test]
fn good_fixture_passes() {
    let findings = run_on_fixture_file("good.rs").unwrap();
    assert_eq!(findings.len(), 0, "unexpected findings: {findings:?}");
}

#[test]
fn bad_muted_on_bg_fails() {
    let findings = run_on_fixture_file("bad_muted.rs").unwrap();
    let msg = &findings.first().expect("a finding").message;
    assert!(msg.contains("muted on bg"), "message: {msg}");
    assert!(msg.contains("dark/neutral"), "message: {msg}");
}

#[test]
fn bad_accent_fg_fails() {
    let findings = run_on_fixture_file("bad_accent_fg.rs").unwrap();
    let msg = &findings.first().expect("a finding").message;
    assert!(msg.contains("accent-fg on accent"), "message: {msg}");
    assert!(msg.contains("dark/neutral"), "message: {msg}");
}

#[test]
fn bad_tint_fails_and_names_tint() {
    let findings = run_on_fixture_file("bad_tint.rs").unwrap();
    let msg = &findings.first().expect("a finding").message;
    assert!(msg.contains("light/warm"), "message: {msg}");
    assert!(msg.contains("text on surface"), "message: {msg}");
}

#[test]
fn missing_prefs_file_passes() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir_all(&root).unwrap();
    // No prefs.rs file
    let findings = run(&root).unwrap();
    assert_eq!(findings.len(), 0);
}

#[test]
fn missing_dark_table_fails_with_clear_message() {
    let findings = run_on_fixture_file("missing_dark.rs").unwrap();
    let msg = &findings.first().expect("a finding").message;
    assert!(
        msg.contains("DARK not found") || msg.contains("table DARK not found"),
        "message: {msg}"
    );
}

// ---- Real repo test ----

#[test]
fn the_real_repo_passes() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir.parent().unwrap(); // workspace root
    let findings = run(root).unwrap();
    assert_eq!(findings.len(), 0, "contrast findings: {findings:#?}");
}
