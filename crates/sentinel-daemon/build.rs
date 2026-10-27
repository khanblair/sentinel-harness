//! The daemon embeds the built UI from `crates/sentinel-ui/dist/`. A fresh clone has no such
//! folder, so this script writes a placeholder page there and the crate still compiles.
//! `trunk build` replaces the placeholder with the real UI.

use std::fs;
use std::io;
use std::path::Path;

const DIST: &str = "../sentinel-ui/dist";
const PLACEHOLDER: &str = "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Sentinel</title></head>\n<body><p>The Sentinel UI is not built in this copy. Build it with <code>cd crates/sentinel-ui &amp;&amp; trunk build</code>, then build the daemon again. <code>sentinel run</code> works without the UI.</p></body>\n</html>\n";

fn main() {
    println!("cargo:rerun-if-changed={DIST}");
    // A failed placeholder only leaves the embed empty. It must not fail the build.
    if let Err(error) = write_placeholder(Path::new(DIST)) {
        eprintln!("could not write the UI placeholder: {error}");
    }
}

fn write_placeholder(dist: &Path) -> io::Result<()> {
    if dist.exists() {
        return Ok(());
    }
    fs::create_dir_all(dist)?;
    fs::write(dist.join("index.html"), PLACEHOLDER)
}
