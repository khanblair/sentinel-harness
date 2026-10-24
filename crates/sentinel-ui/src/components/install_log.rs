//! `InstallLog`: the terminal-style box that shows the install commands and what to do with
//! them. The page cannot run installs, so it copies the commands and says so.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen_futures::JsFuture;

use crate::state::{ToastKind, Ui};

/// One line of the log. `tone` picks the color class: `y` for a command, `m` for a note, `g` for
/// a result that worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub tone: &'static str,
    pub text: String,
}

/// The lines for "Install what is missing": each command, then what to do next.
pub fn install_lines(commands: &[String]) -> Vec<LogLine> {
    let mut lines: Vec<LogLine> = commands
        .iter()
        .map(|command| LogLine {
            tone: "y",
            text: format!("$ {command}"),
        })
        .collect();
    lines.push(LogLine {
        tone: "m",
        text: "Run these in a terminal, then check again.".to_string(),
    });
    lines
}

/// Copies text to the clipboard and says so in a toast. Copying can be blocked, so a refusal
/// gets a plain hint instead of an error.
pub fn copy_text(ui: Ui, text: String) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let clipboard = window.navigator().clipboard();
    spawn_local(async move {
        match JsFuture::from(clipboard.write_text(&text)).await {
            Ok(_) => ui.show_toast("Copied", ToastKind::Ok),
            Err(_) => ui.show_toast(
                "Could not copy. Select the text and copy it.",
                ToastKind::Warn,
            ),
        }
    });
}

#[component]
pub fn InstallLog(#[prop(into)] lines: Signal<Vec<LogLine>>) -> impl IntoView {
    move || {
        let current = lines.get();
        (!current.is_empty()).then(|| {
            view! {
                <div class="term" id="instLog" style="min-height:8em;max-height:14em">
                    {current
                        .into_iter()
                        .map(|line| view! { <div class=line.tone>{line.text}</div> })
                        .collect_view()}
                </div>
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_command_is_a_yellow_prompt_line() {
        let lines = install_lines(&["rustup target add wasm32-unknown-unknown".to_string()]);
        assert_eq!(lines[0].tone, "y");
        assert_eq!(lines[0].text, "$ rustup target add wasm32-unknown-unknown");
    }

    #[test]
    fn the_log_ends_with_the_run_in_a_terminal_note() {
        let lines = install_lines(&[]);
        assert_eq!(
            lines,
            [LogLine {
                tone: "m",
                text: "Run these in a terminal, then check again.".to_string(),
            }]
        );
    }
}
