//! A small starter project to try Sentinel on: a Python greeting with a unittest test and a backlog.
//! The app makes it when the owner has no project yet, and `sentinel starter` makes it on demand.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;
use crate::folders::STARTER_BACKLOG;

const FOLDER_NAME: &str = "Sentinel Starter";

/// The test command the starter uses. Python 3 comes with macOS.
const TEST_COMMAND: [&str; 3] = ["python3", "-m", "unittest"];

const README: &str = "# Sentinel Starter

A small Python project. `greet.py` returns a greeting, and `test_greet.py` checks it.

Run the test from this folder:

    python3 -m unittest

`backlog.md` lists three tasks for Sentinel to work on.
";

const GREET: &str = r#"def greet(name):
    return f"Hello, {name}"
"#;

const TEST_GREET: &str = r#"import unittest

from greet import greet


class GreetTest(unittest.TestCase):
    def test_greets_by_name(self):
        self.assertEqual(greet("Sam"), "Hello, Sam")


if __name__ == "__main__":
    unittest.main()
"#;

const GITIGNORE: &str = ".sentinel/\n__pycache__/\n";

/// Every file of the starter, with its text. The backlog text is shared with the folder check.
const FILES: [(&str, &str); 5] = [
    ("README.md", README),
    ("greet.py", GREET),
    ("test_greet.py", TEST_GREET),
    (".gitignore", GITIGNORE),
    ("backlog.md", STARTER_BACKLOG),
];

/// The first free folder name in `parent`: `Sentinel Starter`, then `Sentinel Starter 2`, and so on.
pub fn next_free_folder(parent: &Path) -> PathBuf {
    let mut candidate = parent.join(FOLDER_NAME);
    let mut number = 2;
    while candidate.exists() {
        candidate = parent.join(format!("{FOLDER_NAME} {number}"));
        number += 1;
    }
    candidate
}

/// Make the starter folder in `parent`, write its files, and commit them on `main`. The commit
/// does not need a git identity on this computer, and it never asks for a signature.
pub fn create_starter_project(parent: &Path) -> std::io::Result<PathBuf> {
    let folder = next_free_folder(parent);
    fs::create_dir_all(&folder)?;
    for (name, text) in FILES {
        fs::write(folder.join(name), text)?;
    }
    git(&folder, &["init", "-b", "main"])?;
    git(&folder, &["add", "-A"])?;
    git(
        &folder,
        &[
            "-c",
            "user.name=Sentinel",
            "-c",
            "user.email=sentinel@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "Starter project",
        ],
    )?;
    Ok(folder)
}

/// Run one git command in `folder`. A failure carries git's own message.
fn git(folder: &Path, args: &[&str]) -> std::io::Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(folder)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(std::io::Error::other(format!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

/// Save the starter's test command in its config, so a run tests it with Python.
pub fn set_starter_test_command(folder: &Path) -> config::Result<()> {
    let path = config::config_path(folder);
    let mut settings = config::load(&path)?;
    settings.test_command = TEST_COMMAND.iter().map(ToString::to_string).collect();
    config::save(&path, &settings)
}

/// What the owner picks when Sentinel has no project to open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstChoice {
    PickFolder,
    MakeStarter,
    Quit,
}

const PICK_LABEL: &str = "Pick a folder";
const STARTER_LABEL: &str = "Make a starter";
const QUIT_LABEL: &str = "Quit";

/// Show the first-run dialog. Closing it, or Quit, stops the app.
pub fn ask_first_choice() -> FirstChoice {
    let result = rfd::MessageDialog::new()
        .set_title("Sentinel")
        .set_description(
            "Pick the folder of a git project, or make a starter project to try Sentinel on.",
        )
        .set_buttons(rfd::MessageButtons::YesNoCancelCustom(
            PICK_LABEL.to_owned(),
            STARTER_LABEL.to_owned(),
            QUIT_LABEL.to_owned(),
        ))
        .show();
    choice_from(&result)
}

/// Map a dialog result to the owner's choice. Platforms report a custom button as its label or as
/// the plain yes, no, and cancel results, so both forms are read.
fn choice_from(result: &rfd::MessageDialogResult) -> FirstChoice {
    match result {
        rfd::MessageDialogResult::Yes => FirstChoice::PickFolder,
        rfd::MessageDialogResult::No => FirstChoice::MakeStarter,
        rfd::MessageDialogResult::Custom(label) if label == PICK_LABEL => FirstChoice::PickFolder,
        rfd::MessageDialogResult::Custom(label) if label == STARTER_LABEL => {
            FirstChoice::MakeStarter
        }
        rfd::MessageDialogResult::Ok
        | rfd::MessageDialogResult::Cancel
        | rfd::MessageDialogResult::Custom(_) => FirstChoice::Quit,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;

    fn git_on_path() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn python_on_path() -> bool {
        Command::new("python3")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[test]
    fn the_first_starter_name_is_plain_then_numbered_from_two() {
        let parent = tempfile::tempdir().unwrap();
        assert_eq!(
            next_free_folder(parent.path()),
            parent.path().join("Sentinel Starter")
        );
        fs::create_dir(parent.path().join("Sentinel Starter")).unwrap();
        fs::create_dir(parent.path().join("Sentinel Starter 2")).unwrap();
        assert_eq!(
            next_free_folder(parent.path()),
            parent.path().join("Sentinel Starter 3")
        );
    }

    #[test]
    fn the_starter_has_its_files_and_the_three_backlog_tasks() {
        let parent = tempfile::tempdir().unwrap();
        let folder = create_starter_project(parent.path()).unwrap();
        for (name, _) in FILES {
            assert!(folder.join(name).is_file(), "{name} is missing");
        }
        let greet = fs::read_to_string(folder.join("greet.py")).unwrap();
        assert!(greet.contains(r#"return f"Hello, {name}""#));
        let backlog = fs::read_to_string(folder.join("backlog.md")).unwrap();
        assert_eq!(backlog.lines().filter(|l| l.starts_with("## ")).count(), 3);
    }

    #[test]
    fn the_starter_is_one_commit_on_main() {
        if !git_on_path() {
            return;
        }
        let parent = tempfile::tempdir().unwrap();
        let folder = create_starter_project(parent.path()).unwrap();
        let log = Command::new("git")
            .args(["log", "--format=%s"])
            .current_dir(&folder)
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&log.stdout), "Starter project\n");
        let status = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&folder)
            .output()
            .unwrap();
        assert!(
            status.stdout.is_empty(),
            "the starter tree must start clean"
        );
    }

    #[test]
    fn the_starter_test_passes_with_python() {
        if !git_on_path() || !python_on_path() {
            return;
        }
        let parent = tempfile::tempdir().unwrap();
        let folder = create_starter_project(parent.path()).unwrap();
        let run = Command::new("python3")
            .args(["-m", "unittest"])
            .current_dir(&folder)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }

    #[test]
    fn the_starter_settings_name_the_python_test_command() {
        let parent = tempfile::tempdir().unwrap();
        let folder = create_starter_project(parent.path()).unwrap();
        set_starter_test_command(&folder).unwrap();
        let saved = config::load(&config::config_path(&folder)).unwrap();
        assert_eq!(saved.test_command, ["python3", "-m", "unittest"]);
    }

    #[test]
    fn each_dialog_answer_maps_to_one_choice() {
        use rfd::MessageDialogResult as Answer;
        assert_eq!(choice_from(&Answer::Yes), FirstChoice::PickFolder);
        assert_eq!(choice_from(&Answer::No), FirstChoice::MakeStarter);
        assert_eq!(
            choice_from(&Answer::Custom(PICK_LABEL.to_owned())),
            FirstChoice::PickFolder
        );
        assert_eq!(
            choice_from(&Answer::Custom(STARTER_LABEL.to_owned())),
            FirstChoice::MakeStarter
        );
        assert_eq!(choice_from(&Answer::Cancel), FirstChoice::Quit);
        assert_eq!(choice_from(&Answer::Ok), FirstChoice::Quit);
    }
}
