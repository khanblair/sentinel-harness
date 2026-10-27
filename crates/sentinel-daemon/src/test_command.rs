//! The command the barrier runs when a repository does not name one. A Rust repo gets
//! `cargo test`, but a Node, Python, Go, or Make project needs its own.

use std::fs;
use std::path::Path;

const PYTHON_MARKERS: [&str; 4] = ["pyproject.toml", "pytest.ini", "setup.cfg", "tox.ini"];
const PYTHON_TEST_DIRS: [&str; 2] = ["tests", "test"];

/// The first rule that matches wins. Project files that name their own test script come first.
pub fn detect(repo: &Path) -> Option<Vec<String>> {
    detect_node(repo)
        .or_else(|| detect_rust(repo))
        .or_else(|| detect_python(repo))
        .or_else(|| detect_go(repo))
        .or_else(|| detect_make(repo))
}

/// The script runs through `sh`, not `npm`, so a machine with Node and no npm still works.
fn detect_node(repo: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(repo.join("package.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
    let script = manifest.get("scripts")?.get("test")?.as_str()?;
    if script.trim().is_empty() {
        return None;
    }
    if repo.join("tests.lock.json").is_file() {
        // The locked test folders take no new file, so the tests of the tasks are in their own
        // folder. They run after the project's own suite.
        let both = format!("({}) && {OWN_TESTS_CLAUSE}", script.trim());
        return Some(words(&["sh", "-c", &both]));
    }
    Some(words(&["sh", "-c", script]))
}

/// Runs the tests that the tasks wrote in `sentinel-tests/`, when there are any.
const OWN_TESTS_CLAUSE: &str =
    "if ls sentinel-tests/*.test.js >/dev/null 2>&1; then node --test sentinel-tests/*.test.js; fi";

fn detect_rust(repo: &Path) -> Option<Vec<String>> {
    repo.join("Cargo.toml")
        .is_file()
        .then(|| words(&["cargo", "test"]))
}

fn detect_python(repo: &Path) -> Option<Vec<String>> {
    let marked = PYTHON_MARKERS.iter().any(|name| repo.join(name).is_file());
    let has_tests = PYTHON_TEST_DIRS
        .iter()
        .any(|dir| folder_has_python_test(&repo.join(dir)));
    (marked || has_tests).then(|| words(&["python3", "-m", "pytest", "-q"]))
}

fn folder_has_python_test(folder: &Path) -> bool {
    let Ok(entries) = fs::read_dir(folder) else {
        return false;
    };
    entries
        .flatten()
        .any(|entry| is_python_test_name(&entry.file_name().to_string_lossy()))
}

fn is_python_test_name(name: &str) -> bool {
    let path = Path::new(name);
    let is_python = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("py"));
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    is_python && (stem.starts_with("test_") || stem.ends_with("_test"))
}

fn detect_go(repo: &Path) -> Option<Vec<String>> {
    repo.join("go.mod")
        .is_file()
        .then(|| words(&["go", "test", "./..."]))
}

fn detect_make(repo: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(repo.join("Makefile")).ok()?;
    text.lines()
        .any(|line| line.starts_with("test:"))
        .then(|| words(&["make", "test"]))
}

fn words(parts: &[&str]) -> Vec<String> {
    parts.iter().map(ToString::to_string).collect()
}

/// Splits a command text into words. Single and double quotes group words; there are no
/// escapes. Runs of spaces separate words, and an empty text gives no words.
pub fn split_command(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for ch in text.chars() {
        match (quote, ch) {
            (Some(open), close) if open == close => quote = None,
            (Some(_), _) => word.push(ch),
            (None, '\'' | '"') => {
                quote = Some(ch);
                in_word = true;
            }
            (None, _) if ch.is_whitespace() && in_word => {
                parts.push(std::mem::take(&mut word));
                in_word = false;
            }
            (None, _) if ch.is_whitespace() => {}
            (None, _) => {
                word.push(ch);
                in_word = true;
            }
        }
    }
    if in_word {
        parts.push(word);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_locked_node_repository_also_runs_the_tests_of_the_tasks() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("package.json"),
            r#"{"scripts": {"test": "node scripts/run-suite.js test"}}"#,
        )
        .unwrap();
        fs::write(folder.path().join("tests.lock.json"), "{}").unwrap();
        let command = detect(folder.path()).unwrap();
        assert_eq!(command[..2], ["sh", "-c"]);
        assert!(command[2].starts_with("(node scripts/run-suite.js test) && "));
        assert!(command[2].contains("node --test sentinel-tests/*.test.js"));
    }

    fn write(dir: &Path, name: &str, text: &str) {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text).unwrap();
    }

    #[test]
    fn detect_returns_none_for_an_empty_folder() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect(dir.path()), None);
    }

    #[test]
    fn detect_runs_the_package_json_test_script_through_sh() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "package.json",
            r#"{"scripts":{"test":"node run.js"}}"#,
        );
        assert_eq!(
            detect(dir.path()),
            Some(words(&["sh", "-c", "node run.js"]))
        );
    }

    #[test]
    fn detect_skips_a_package_json_without_a_test_script() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", r#"{"scripts":{"build":"tsc"}}"#);
        write(dir.path(), "Cargo.toml", "[package]\n");
        assert_eq!(detect(dir.path()), Some(words(&["cargo", "test"])));
    }

    #[test]
    fn detect_prefers_package_json_over_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", "[package]\n");
        write(
            dir.path(),
            "package.json",
            r#"{"scripts":{"test":"vitest"}}"#,
        );
        assert_eq!(detect(dir.path()), Some(words(&["sh", "-c", "vitest"])));
    }

    #[test]
    fn detect_ignores_a_blank_package_json_test_script() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "package.json", r#"{"scripts":{"test":"  "}}"#);
        assert_eq!(detect(dir.path()), None);
    }

    #[test]
    fn detect_picks_cargo_test_for_a_cargo_manifest() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", "[package]\n");
        assert_eq!(detect(dir.path()), Some(words(&["cargo", "test"])));
    }

    #[test]
    fn detect_picks_pytest_for_a_pyproject_file() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "pyproject.toml", "[project]\n");
        assert_eq!(
            detect(dir.path()),
            Some(words(&["python3", "-m", "pytest", "-q"]))
        );
    }

    #[test]
    fn detect_picks_pytest_for_a_test_file_in_the_tests_folder() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "tests/test_app.py", "");
        assert_eq!(
            detect(dir.path()),
            Some(words(&["python3", "-m", "pytest", "-q"]))
        );
    }

    #[test]
    fn detect_ignores_python_files_that_are_not_tests() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "test/helpers.py", "");
        assert_eq!(detect(dir.path()), None);
    }

    #[test]
    fn detect_picks_go_test_for_a_go_module() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "go.mod", "module example\n");
        assert_eq!(detect(dir.path()), Some(words(&["go", "test", "./..."])));
    }

    #[test]
    fn detect_picks_make_test_only_when_a_test_target_exists() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "Makefile",
            "build:\n\tcc main.c\n\ntest:\n\t./run\n",
        );
        assert_eq!(detect(dir.path()), Some(words(&["make", "test"])));

        let other = tempfile::tempdir().unwrap();
        write(other.path(), "Makefile", "build:\n\tcc main.c\n");
        assert_eq!(detect(other.path()), None);
    }

    #[test]
    fn split_command_splits_on_spaces_and_collapses_extra_spaces() {
        assert_eq!(
            split_command("  node   scripts/run-suite.js test  "),
            words(&["node", "scripts/run-suite.js", "test"])
        );
    }

    #[test]
    fn split_command_keeps_quoted_text_as_one_word() {
        assert_eq!(
            split_command(r#"sh -c "echo a b" 'x y'"#),
            words(&["sh", "-c", "echo a b", "x y"])
        );
    }

    #[test]
    fn split_command_keeps_the_other_quote_inside_quotes() {
        assert_eq!(
            split_command(r#"echo "it's ok""#),
            words(&["echo", "it's ok"])
        );
    }

    #[test]
    fn split_command_gives_no_words_for_blank_text() {
        assert_eq!(split_command(""), Vec::<String>::new());
        assert_eq!(split_command("   "), Vec::<String>::new());
    }
}
