//! A file checkpoint: one changed path and the short commit that holds it.

use leptos::prelude::*;

use crate::components::icon::Icon;

/// The first seven characters of a commit id, as people read it.
pub fn short_commit(commit: &str) -> String {
    commit.chars().take(7).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_commit_keeps_seven_characters() {
        assert_eq!(short_commit("abc1234def5678"), "abc1234");
        assert_eq!(short_commit("abc"), "abc");
    }
}
