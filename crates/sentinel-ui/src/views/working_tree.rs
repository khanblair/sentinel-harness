//! The "Working tree" section at the top of the Changes panel: the files git reports right now,
//! each with its diff one click away (`GET /api/changes`).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::ChangedFile;

use crate::api;
use crate::components::diff_view::DiffSlot;
use crate::components::file_change_card::WorkingFileRow;
use crate::components::icon::Icon;
use crate::state::use_ui;
use crate::views::live_fetch::keep_live;

/// The heading of the section: `Working tree · 3 files`.
pub fn heading_text(count: usize) -> String {
    match count {
        1 => "Working tree · 1 file".to_owned(),
        many => format!("Working tree · {many} files"),
    }
}

fn load_files(files: RwSignal<Option<Vec<ChangedFile>>>, slot: DiffSlot) {
    spawn_local(async move {
        if let Ok(list) = api::get::<Vec<ChangedFile>>("/api/changes").await {
            files.set(Some(list));
            slot.refresh_open();
        }
    });
}

#[component]
pub fn WorkingTree() -> impl IntoView {
    let ui = use_ui();
    let files = RwSignal::new(None::<Vec<ChangedFile>>);
    let slot = DiffSlot::new();
    keep_live(ui, move || load_files(files, slot));
    view! {
        <div class="psec">
            <h5>{move || heading_text(files.with(|list| list.as_ref().map_or(0, Vec::len)))}</h5>
            {move || match files.get() {
                None => view! { <div class="note">"Reading the working tree."</div> }.into_any(),
                Some(list) if list.is_empty() => view! {
                    <div class="note">
                        <Icon name="info" size=14 />
                        <span>"The working tree is clean."</span>
                    </div>
                }.into_any(),
                Some(list) => list
                    .into_iter()
                    .map(|file| view! { <WorkingFileRow file=file diffs=slot /> })
                    .collect_view()
                    .into_any(),
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_counts_files_with_the_right_plural() {
        assert_eq!(heading_text(0), "Working tree · 0 files");
        assert_eq!(heading_text(1), "Working tree · 1 file");
        assert_eq!(heading_text(3), "Working tree · 3 files");
    }
}
