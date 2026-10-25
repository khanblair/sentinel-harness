//! `DiffView`: a unified diff in a `<pre>`. Added lines show in the ok color, removed lines in the
//! danger color. `DiffSlot` holds the one file whose diff is open in the Changes panel.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::ChangeDiff;

use crate::api;

/// What one line of a diff is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// The `---` and `+++` lines and anything else before the first hunk.
    Header,
    Hunk,
    Added,
    Removed,
    Context,
}

/// Splits diff text into lines and names each one. Lines before the first `@@` are the header,
/// so a removed line that starts with `--` is still a removed line inside a hunk.
pub fn diff_lines(text: &str) -> Vec<(DiffKind, String)> {
    let mut in_hunk = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let kind = if line.starts_with("@@") {
            in_hunk = true;
            DiffKind::Hunk
        } else if !in_hunk {
            DiffKind::Header
        } else if line.starts_with('+') {
            DiffKind::Added
        } else if line.starts_with('-') {
            DiffKind::Removed
        } else {
            DiffKind::Context
        };
        lines.push((kind, line.to_owned()));
    }
    lines
}

/// The inline style of one diff line. Each line is a block so the colors stack.
fn line_style(kind: DiffKind) -> &'static str {
    match kind {
        DiffKind::Added => "display:block;color:var(--ok)",
        DiffKind::Removed => "display:block;color:var(--danger)",
        DiffKind::Hunk | DiffKind::Header => "display:block;color:var(--muted)",
        DiffKind::Context => "display:block",
    }
}

#[component]
pub fn DiffView(lines: Vec<(DiffKind, String)>, truncated: bool) -> impl IntoView {
    view! {
        <pre class="mono" style="margin:.4em 0 0;padding:.5em .6em;border:1px solid var(--border);border-radius:var(--r);background:var(--surface2);max-height:40vh;overflow:auto;white-space:pre;font-size:.82em;line-height:1.5">
            {lines.into_iter().map(|(kind, line)| view! {
                <span style=line_style(kind)>{line}</span>
            }).collect_view()}
        </pre>
        <Show when=move || truncated fallback=|| ()>
            <div class="sub" style="margin-top:.3em">"The diff is cut at 200 KB. Open the file to see the rest."</div>
        </Show>
    }
}

/// The open diff of the Changes panel: which file is open, and its diff once it loads.
#[derive(Debug, Clone, Copy)]
pub struct DiffSlot {
    pub selected: RwSignal<Option<String>>,
    pub diff: RwSignal<Option<ChangeDiff>>,
}

impl Default for DiffSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffSlot {
    pub fn new() -> Self {
        Self {
            selected: RwSignal::new(None),
            diff: RwSignal::new(None),
        }
    }

    /// Opens the diff of `path`, or closes it when it is already open.
    pub fn toggle(self, path: String) {
        let closing = self
            .selected
            .with_untracked(|open| open.as_deref() == Some(path.as_str()));
        if closing {
            self.selected.set(None);
            self.diff.set(None);
            return;
        }
        self.selected.set(Some(path.clone()));
        self.diff.set(None);
        self.fetch(path);
    }

    /// Fetches the open file's diff again, so the diff follows the working tree.
    pub fn refresh_open(self) {
        if let Some(path) = self.selected.get_untracked() {
            self.fetch(path);
        }
    }

    fn fetch(self, path: String) {
        spawn_local(async move {
            let url = format!("/api/changes/diff?path={}", encode(&path));
            let Ok(diff) = api::get::<ChangeDiff>(&url).await else {
                return;
            };
            // A late answer for a file the user already closed or left is dropped.
            let still_open = self
                .selected
                .with_untracked(|open| open.as_deref() == Some(diff.path.as_str()));
            if still_open {
                self.diff.set(Some(diff));
            }
        });
    }
}

fn encode(text: &str) -> String {
    String::from(js_sys::encode_uri_component(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_before_the_first_hunk_are_header() {
        let text = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-old\n+new\n kept\n";
        let kinds: Vec<DiffKind> = diff_lines(text).into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(
            kinds,
            vec![
                DiffKind::Header,
                DiffKind::Header,
                DiffKind::Header,
                DiffKind::Hunk,
                DiffKind::Removed,
                DiffKind::Added,
                DiffKind::Context,
            ]
        );
    }

    #[test]
    fn a_removed_line_that_starts_with_dashes_stays_removed() {
        let lines = diff_lines("@@ -1 +0,0 @@\n--- old comment\n");
        assert_eq!(lines[1].0, DiffKind::Removed);
        assert_eq!(lines[1].1, "--- old comment");
    }

    #[test]
    fn an_empty_diff_has_no_lines() {
        assert_eq!(diff_lines("").len(), 0);
    }
}
