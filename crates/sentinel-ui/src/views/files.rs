//! The Files panel: the repository files with reserved-file markers and a filter
//! (`docs/ui-rules.md`, Views). The daemon has no `/api/files` route yet, so a 404 shows a note.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::Snapshot;

use crate::api::{self, ApiFailure};
use crate::components::icon::Icon;
use crate::components::reserved_row::{ReservedRow, ReservedRowModel};
use crate::state::use_ui;
use crate::views::reserved::group_reservations;

/// What the file list request returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    Loading,
    Ready(Vec<String>),
    /// The daemon does not have the route yet.
    Missing,
    Failed(String),
}

pub fn listing_from(result: Result<Vec<String>, ApiFailure>) -> Listing {
    match result {
        Ok(files) => Listing::Ready(files),
        Err(ApiFailure::Status { code: 404, .. }) => Listing::Missing,
        Err(failure) => Listing::Failed(failure.to_string()),
    }
}

/// Case-insensitive substring match. An empty query keeps every file.
pub fn filter_files(files: &[String], query: &str) -> Vec<String> {
    let needle = query.trim().to_ascii_lowercase();
    files
        .iter()
        .filter(|path| needle.is_empty() || path.to_ascii_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// The marker row for one file: its holder and queue, or nothing when no task touches it.
pub fn marker_row(path: String, reserved: &[ReservedRowModel]) -> ReservedRowModel {
    reserved
        .iter()
        .find(|row| row.path == path)
        .cloned()
        .unwrap_or(ReservedRowModel {
            path,
            holder: None,
            waiting: Vec::new(),
        })
}

fn file_rows(snapshot: &Snapshot, files: &[String], query: &str) -> Vec<ReservedRowModel> {
    let reserved = group_reservations(&snapshot.reservations);
    filter_files(files, query)
        .into_iter()
        .map(|path| marker_row(path, &reserved))
        .collect()
}

#[component]
fn Listed(listing: Listing, query: RwSignal<String>) -> impl IntoView {
    let ui = use_ui();
    match listing {
        Listing::Loading => view! { <div class="note">"Loading files."</div> }.into_any(),
        Listing::Missing => view! {
            <div class="note"><Icon name="info" size=14 /><span>"The file list is not available yet."</span></div>
        }
        .into_any(),
        Listing::Failed(text) => view! { <div class="note" style="color:var(--danger)">{text}</div> }
            .into_any(),
        Listing::Ready(files) => view! {
            <div class="tree">
                {move || {
                    ui.snapshot
                        .with(|snapshot| file_rows(snapshot, &files, &query.get()))
                        .into_iter()
                        .map(|row| view! { <ReservedRow row=row /> })
                        .collect_view()
                }}
            </div>
        }
        .into_any(),
    }
}

#[component]
pub fn FilesPanel() -> impl IntoView {
    let query = RwSignal::new(String::new());
    let listing = RwSignal::new(Listing::Loading);
    spawn_local(async move {
        listing.set(listing_from(api::get::<Vec<String>>("/api/files").await));
    });
    view! {
        <input class="field" placeholder="Filter files" aria-label="Filter files"
            style="margin-bottom:.8em"
            prop:value=move || query.get()
            on:input=move |event| query.set(event_target_value(&event)) />
        {move || view! { <Listed listing=listing.get() query=query /> }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::{Reservation, ReservationId, ReservationState, TaskId};
    use time::OffsetDateTime;

    #[test]
    fn filter_ignores_case_and_keeps_all_on_empty_query() {
        let files = vec!["src/API/a.ts".to_owned(), "README.md".to_owned()];
        assert_eq!(filter_files(&files, "api"), vec!["src/API/a.ts".to_owned()]);
        assert_eq!(filter_files(&files, "  "), files);
    }

    #[test]
    fn a_404_means_the_list_is_not_available_yet() {
        let missing = listing_from(Err(ApiFailure::Status {
            code: 404,
            message: "not found".to_owned(),
        }));
        assert_eq!(missing, Listing::Missing);
        assert_eq!(
            listing_from(Ok(vec!["a".to_owned()])),
            Listing::Ready(vec!["a".to_owned()])
        );
    }

    #[test]
    fn other_failures_keep_their_message() {
        let failed = listing_from(Err(ApiFailure::Network("offline".to_owned())));
        assert!(matches!(failed, Listing::Failed(text) if text.contains("offline")));
    }

    #[test]
    fn marker_row_takes_the_holder_of_a_reserved_file() {
        let reservation = Reservation {
            id: ReservationId::new(1),
            task_id: TaskId::new("T-01"),
            path: "src/a.rs".to_owned(),
            state: ReservationState::Granted,
            reserved_at: OffsetDateTime::UNIX_EPOCH,
            released_at: None,
        };
        let reserved = group_reservations(&[reservation]);
        let held = marker_row("src/a.rs".to_owned(), &reserved);
        assert_eq!(held.holder.as_deref(), Some("T-01"));
        let free = marker_row("src/b.rs".to_owned(), &reserved);
        assert_eq!(free.holder, None);
    }
}
