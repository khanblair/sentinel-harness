//! The daemon embeds the built UI from `crates/sentinel-ui/dist/`. On a fresh clone that folder
//! does not exist yet, so this script puts a one-page placeholder there and the crate still
//! compiles. `trunk build` replaces it with the real UI.

use std::fs;
use std::path::Path;

const PLACEHOLDER: &str = "<!doctype html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><title>Sentinel</title></head>\n<body><p>The UI has not been built. Run <code>trunk build --release</code> in crates/sentinel-ui, then rebuild the daemon.</p></body>\n</html>\n";

fn main() {
    let dist = Path::new("../sentinel-ui/dist");
    let index = dist.join("index.html");
    if !index.exists() {
        // A failure here only means the embed has nothing to serve. The compiler says so next.
        let _ = fs::create_dir_all(dist);
        let _ = fs::write(&index, PLACEHOLDER);
    }
    println!("cargo:rerun-if-changed=../sentinel-ui/dist/index.html");
}
