//! Repo automation: `cargo xtask <command>`.

mod ban;
mod ci;
mod contrast;
mod docs;
mod dup;
mod files;
mod finding;
mod rust_scan;
mod slop;
mod smell;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    let Some(cmd) = args.next() else {
        bail!(
            "usage: cargo xtask <ci|checks|slop-check|smell-check|dup-check|doc-check|ban-check|contrast-check> [--root DIR]"
        );
    };
    let root = match (args.next().as_deref(), args.next()) {
        (Some("--root"), Some(dir)) => PathBuf::from(dir),
        _ => repo_root(),
    };
    match cmd.as_str() {
        "ci" => ci::run(&root),
        "checks" => ci::run_checks(&root),
        name => ci::run_one(name, &root),
    }
}

fn repo_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map_or(manifest.clone(), PathBuf::from)
}
