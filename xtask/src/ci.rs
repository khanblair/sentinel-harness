//! Runs the checks: `cargo xtask ci`, `cargo xtask checks`, or one check by name.

use std::path::Path;
use std::process::{Command, ExitCode};

use anyhow::{Result, bail};

use crate::finding::CheckResult;
use crate::{ban, contrast, docs, dup, slop, smell};

type CheckFn = fn(&Path) -> CheckResult;

/// Checks that only read files. Order matches `docs/architecture.md`, Automated Quality Checks.
const CHECKS: [(&str, CheckFn); 6] = [
    ("smell-check", smell::run),
    ("dup-check", dup::run),
    ("slop-check", slop::run),
    ("doc-check", docs::run),
    ("ban-check", ban::run),
    ("contrast-check", contrast::run),
];

/// A cargo command, skipped when `needs` names a path that does not exist yet.
struct CargoStep {
    name: &'static str,
    args: &'static [&'static str],
    needs: Option<&'static str>,
}

const FIRST_STEPS: [CargoStep; 3] = [
    CargoStep {
        name: "fmt",
        args: &["fmt", "--all", "--check"],
        needs: None,
    },
    CargoStep {
        name: "clippy",
        args: &[
            "clippy",
            "--workspace",
            "--exclude",
            "sentinel-ui",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        needs: None,
    },
    CargoStep {
        name: "nextest",
        args: &[
            "nextest",
            "run",
            "--workspace",
            "--exclude",
            "sentinel-ui",
            "--no-tests=pass",
        ],
        needs: None,
    },
];

const TOOL_STEPS: [CargoStep; 3] = [
    CargoStep {
        name: "deny",
        args: &["deny", "check"],
        needs: None,
    },
    CargoStep {
        name: "machete",
        args: &["machete"],
        needs: None,
    },
    CargoStep {
        name: "clippy-wasm",
        args: &[
            "clippy",
            "-p",
            "sentinel-ui",
            "--target",
            "wasm32-unknown-unknown",
            "--",
            "-D",
            "warnings",
        ],
        needs: Some("crates/sentinel-ui"),
    },
];

/// `cargo xtask ci`: every cargo step, then every file check. Runs all, reports all.
pub fn run(root: &Path) -> Result<ExitCode> {
    let mut failed = Vec::new();
    for step in FIRST_STEPS.iter().chain(TOOL_STEPS.iter()) {
        if !run_cargo(step, root)? {
            failed.push(step.name);
        }
    }
    failed.extend(failed_checks(root)?);
    summarize(&failed)
}

/// `cargo xtask checks`: only the file checks. Fast, no cargo build.
pub fn run_checks(root: &Path) -> Result<ExitCode> {
    summarize(&failed_checks(root)?)
}

/// `cargo xtask <name>`: one file check.
pub fn run_one(name: &str, root: &Path) -> Result<ExitCode> {
    let Some((_, check)) = CHECKS.iter().find(|(n, _)| *n == name) else {
        bail!(
            "unknown command `{name}`. Use ci, checks, or one of: {}",
            names()
        );
    };
    let findings = check(root)?;
    for f in &findings {
        println!("{f}");
    }
    summarize(&if findings.is_empty() {
        vec![]
    } else {
        vec![name]
    })
}

fn names() -> String {
    CHECKS
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

fn failed_checks(root: &Path) -> Result<Vec<&'static str>> {
    let mut failed = Vec::new();
    for (name, check) in CHECKS {
        let findings = check(root)?;
        for f in &findings {
            println!("{f}");
        }
        println!(
            "{name}: {}",
            if findings.is_empty() { "ok" } else { "FAILED" }
        );
        if !findings.is_empty() {
            failed.push(name);
        }
    }
    Ok(failed)
}

fn run_cargo(step: &CargoStep, root: &Path) -> Result<bool> {
    if step.needs.is_some_and(|p| !root.join(p).exists()) {
        println!("{}: skipped (not created yet)", step.name);
        return Ok(true);
    }
    let ok = Command::new("cargo")
        .args(step.args)
        .current_dir(root)
        .status()?
        .success();
    println!("{}: {}", step.name, if ok { "ok" } else { "FAILED" });
    Ok(ok)
}

fn summarize(failed: &[&str]) -> Result<ExitCode> {
    if failed.is_empty() {
        println!("all checks passed");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("failed: {}", failed.join(", "));
        Ok(ExitCode::FAILURE)
    }
}
