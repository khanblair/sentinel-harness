//! `TaskCard`: one task on the Board. Click opens the detail inline under the card
//! (`docs/ui-registry.md`, Board and Tasks).

use leptos::prelude::*;
use sentinel_core::TaskStatus;

use crate::components::status_badge::StatusBadge;

/// Everything a card shows, built from a task and the event log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardModel {
    pub id: String,
    pub title: String,
    pub body: String,
    pub status: TaskStatus,
    pub tier: &'static str,
    pub owner: Option<String>,
    pub attempt: u32,
    pub paths: Vec<String>,
    pub notes: usize,
    pub flag: Option<String>,
}

/// `1 path`, `3 paths`.
pub fn path_count_text(count: usize) -> String {
    if count == 1 {
        "1 path".to_owned()
    } else {
        format!("{count} paths")
    }
}

/// `attempt 2`, or nothing before the first attempt.
pub fn attempt_text(attempt: u32) -> Option<String> {
    (attempt > 0).then(|| format!("attempt {attempt}"))
}

/// `1 quality note`, `2 quality notes`, or nothing when there are none.
pub fn note_count_text(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 quality note".to_owned()),
        many => Some(format!("{many} quality notes")),
    }
}

#[component]
pub fn TaskCard(card: CardModel) -> impl IntoView {
    let open = RwSignal::new(false);
    let CardModel {
        id,
        title,
        body,
        status,
        tier,
        owner,
        attempt,
        paths,
        notes,
        flag,
    } = card;
    let paths_label = path_count_text(paths.len());
    let meta_notes = note_count_text(notes);
    view! {
        <div class="tcard" role="button" tabindex="0" style="cursor:pointer"
            aria-expanded=move || open.get().to_string()
            on:click=move |_| open.update(|value| *value = !*value)>
            <div class="row sp">
                <span class="id">{id}</span>
                <StatusBadge status=status />
            </div>
            <div class="ttl">{title}</div>
            <div class="meta">
                {owner.map(|agent| view! { <span class="row" style="gap:.3em">{agent}</span> })}
                <span class="chip">{tier}</span>
                <span>{paths_label}</span>
                {attempt_text(attempt).map(|text| view! { <span>{text}</span> })}
                {meta_notes.map(|text| view! { <span class="chip">{text}</span> })}
                {flag.map(|text| view! { <span class="chip a">{text}</span> })}
            </div>
            <CardDetail open=open body=body paths=paths />
        </div>
    }
}

/// The body and the file list. Shown only while the card is open.
#[component]
fn CardDetail(open: RwSignal<bool>, body: String, paths: Vec<String>) -> impl IntoView {
    view! {
        <Show when=move || open.get() fallback=|| ()>
            <div class="sub" style="margin-top:.6em;white-space:pre-wrap">{body.clone()}</div>
            <div class="tree" style="margin-top:.5em">
                {paths.iter().map(|path| view! { <div class="tr"><span class="nm">{path.clone()}</span></div> }).collect_view()}
            </div>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_count_uses_singular_for_one() {
        assert_eq!(path_count_text(1), "1 path");
        assert_eq!(path_count_text(0), "0 paths");
        assert_eq!(path_count_text(3), "3 paths");
    }

    #[test]
    fn attempt_text_hides_zero_attempts() {
        assert_eq!(attempt_text(0), None);
        assert_eq!(attempt_text(2).as_deref(), Some("attempt 2"));
    }

    #[test]
    fn note_count_is_hidden_when_empty() {
        assert_eq!(note_count_text(0), None);
        assert_eq!(note_count_text(1).as_deref(), Some("1 quality note"));
        assert_eq!(note_count_text(4).as_deref(), Some("4 quality notes"));
    }
}
