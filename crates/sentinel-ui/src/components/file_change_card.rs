//! A file checkpoint: one changed path and the short commit that holds it. Also the row of one
//! file in the Changes panel's working tree, which opens its diff inline.

use leptos::prelude::*;
use sentinel_core::api::ChangedFile;

use crate::components::diff_view::{DiffSlot, DiffView, diff_lines};
use crate::components::icon::Icon;
use crate::components::reserved_row::split_path;

/// The first seven characters of a commit id, as people read it.
pub fn short_commit(commit: &str) -> String {
    commit.chars().take(7).collect()
}

/// The word the Changes panel shows for a status from `GET /api/changes`.
pub fn status_label(status: &str) -> &'static str {
    match status {
        "added" => "Added",
        "deleted" => "Deleted",
        "renamed" => "Renamed",
        "untracked" => "New",
        "modified" => "Modified",
        _ => "Changed",
    }
}

#[component]
pub fn FileChangeCard(#[prop(into)] path: String, commit: String) -> impl IntoView {
    let short = short_commit(&commit);
    view! {
        <div class="filecard">
            <Icon name="file" size=15 />
            <span class="p mono">{path}</span>
            <span class="mono" title=commit>{short}</span>
        </div>
    }
}

/// One file of the working tree. A click opens its diff under the row, and a second click closes it.
#[component]
pub fn WorkingFileRow(file: ChangedFile, diffs: DiffSlot) -> impl IntoView {
    let ChangedFile {
        path,
        status,
        added,
        removed,
    } = file;
    let label = status_label(&status);
    let (dir, name) = split_path(&path);
    let open_key = path.clone();
    let is_open = move || {
        diffs
            .selected
            .with(|open| open.as_deref() == Some(open_key.as_str()))
    };
    let diff_key = path.clone();
    let click_path = path.clone();
    view! {
        <div class="tr" role="button" tabindex="0" style="cursor:pointer"
            aria-expanded=move || is_open().to_string()
            on:click=move |_| diffs.toggle(click_path.clone())>
            <span class="chip">{label}</span>
            <span class="nm mono"><span style="opacity:.55">{dir}</span>{name}</span>
            <span class="mono" style="color:var(--ok)">{format!("+{added}")}</span>
            <span class="mono" style="color:var(--danger)">{format!("-{removed}")}</span>
        </div>
        {move || {
            diffs
                .diff
                .get()
                .filter(|diff| diff.path == diff_key)
                .map(|diff| {
                    let lines = diff_lines(&diff.diff);
                    view! { <DiffView lines=lines truncated=diff.truncated /> }
                })
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_commit_keeps_seven_characters() {
        assert_eq!(short_commit("abc1234def5678"), "abc1234");
        assert_eq!(short_commit("abc"), "abc");
    }

    #[test]
    fn status_words_get_plain_labels() {
        assert_eq!(status_label("untracked"), "New");
        assert_eq!(status_label("deleted"), "Deleted");
        assert_eq!(status_label("modified"), "Modified");
        assert_eq!(status_label("something new"), "Changed");
    }
}
