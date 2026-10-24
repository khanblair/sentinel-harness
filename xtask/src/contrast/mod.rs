//! Contrast check: design token pairs meet the contrast rules of `docs/ui-tokens.md`.

mod color;
mod parse;
mod rules;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;

use crate::finding::{CheckResult, Finding};

const CHECK: &str = "contrast-check";
const PREFS_PATH: &str = "crates/sentinel-ui/src/prefs.rs";

/// Token name to `#RRGGBB`.
type Palette = BTreeMap<String, String>;

/// One theme variant: a name such as `dark/warm` and every color token it has.
struct Variant {
    name: String,
    colors: Palette,
}

pub fn run(root: &Path) -> CheckResult {
    let prefs_file = root.join(PREFS_PATH);
    if !prefs_file.exists() {
        // The UI crate does not exist yet, so there is nothing to check.
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&prefs_file)
        .with_context(|| format!("could not read {}", prefs_file.display()))?;
    let tables = match parse::read_tables(&text) {
        Ok(tables) => tables,
        Err(problems) => return Ok(problems_to_findings(&problems)),
    };
    let mut findings = Vec::new();
    for variant in variants(&tables) {
        findings.extend(rules::check_variant(&prefs_file, &variant));
    }
    Ok(findings)
}

fn problems_to_findings(problems: &[String]) -> Vec<Finding> {
    problems
        .iter()
        .map(|message| Finding::new(CHECK, PREFS_PATH, 0, message.clone()))
        .collect()
}

/// Dark and light neutral, then every tint laid over its neutral base.
fn variants(tables: &parse::Tables) -> Vec<Variant> {
    let mut all = vec![
        Variant {
            name: "dark/neutral".to_owned(),
            colors: tables.dark.clone(),
        },
        Variant {
            name: "light/neutral".to_owned(),
            colors: tables.light.clone(),
        },
    ];
    for (theme, base, tints) in [
        ("dark", &tables.dark, &tables.dark_tints),
        ("light", &tables.light, &tables.light_tints),
    ] {
        for (tint, colors) in tints {
            let mut merged = base.clone();
            merged.extend(colors.clone());
            all.push(Variant {
                name: format!("{theme}/{tint}"),
                colors: merged,
            });
        }
    }
    all
}

#[cfg(test)]
mod tests;
