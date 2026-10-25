//! The Brain panel: a header with the mode, the note count, and the vault path, then three tabs
//! (Notes, Graph, Proposals). Every Brain call that answers 404 replaces the whole panel with the
//! "not available yet" note (`docs/architecture.md`, API).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{SeedReport, SetBrainMode};
use sentinel_core::brain_api::{GraphView as GraphData, NoteDetail, NoteList, ProposalView};
use sentinel_core::{BrainMode, NoteId};
use wasm_bindgen_futures::JsFuture;

use crate::api::{self, ApiFailure};
use crate::brain::{
    BrainTab, LoadState, SWITCH_TO_LEARN_LABEL, UNAVAILABLE, care_done_text, is_missing, mode_name,
    mode_needs_switch, mode_sentence, proposals_tab_label, seed_done_text,
};
use crate::components::icon::Icon;
use crate::components::top_bar::settle;
use crate::state::{self, ToastKind, Ui, use_ui};
use crate::views::brain_graph::GraphTab;
use crate::views::brain_notes::NotesTab;
use crate::views::brain_proposals::ProposalsTab;

/// The signals every Brain view shares. `revision` goes up after each change, so the open views
/// load again.
#[derive(Clone, Copy)]
pub struct BrainCtx {
    pub ui: Ui,
    pub tab: RwSignal<BrainTab>,
    pub open_note: RwSignal<Option<NoteId>>,
    pub missing: RwSignal<bool>,
    pub revision: RwSignal<u32>,
    pub vault_path: RwSignal<String>,
    pub note_count: RwSignal<usize>,
    pub open_proposals: RwSignal<usize>,
    /// The note graph, shared by the Notes tab (link counts) and the Graph tab.
    pub graph: RwSignal<LoadState<GraphData>>,
    /// True while the notes are being built from the project.
    pub seeding: RwSignal<bool>,
}

impl BrainCtx {
    pub fn new(ui: Ui) -> Self {
        Self {
            ui,
            tab: RwSignal::new(BrainTab::Notes),
            open_note: RwSignal::new(None),
            missing: RwSignal::new(false),
            revision: RwSignal::new(0),
            vault_path: RwSignal::new(String::new()),
            note_count: RwSignal::new(0),
            open_proposals: RwSignal::new(0),
            graph: RwSignal::new(LoadState::Loading),
            seeding: RwSignal::new(false),
        }
    }
}

/// Ask the open views to load again.
pub fn bump(ctx: BrainCtx) {
    ctx.revision.update(|value| *value += 1);
}

/// A 404 means this build has no such route, so the panel shows the note. Anything else is a
/// toast with the daemon's message.
pub fn report_failure(ui: Ui, missing: RwSignal<bool>, failure: &ApiFailure) {
    if is_missing(failure) {
        missing.set(true);
    } else {
        ui.show_toast(failure_text(failure), ToastKind::Danger);
    }
}

/// The daemon's own message for a status error, otherwise the full failure text.
pub fn failure_text(failure: &ApiFailure) -> String {
    match failure {
        ApiFailure::Status { message, .. } => message.clone(),
        ApiFailure::Network(_) | ApiFailure::Decode(_) => failure.to_string(),
    }
}

/// A failed load shows its text in the view, and a toast too, so the page never looks empty.
pub fn fail_load<T: Send + Sync + 'static>(
    ctx: BrainCtx,
    state: RwSignal<LoadState<T>>,
    failure: &ApiFailure,
) {
    report_failure(ctx.ui, ctx.missing, failure);
    state.set(LoadState::Failed(failure_text(failure)));
}

/// The line a list or a note shows until it has an answer, or the text of its failure. A loaded
/// list shows nothing here.
pub fn load_notice<T>(state: &LoadState<T>, loading: &'static str) -> AnyView {
    match state {
        LoadState::Loading => view! { <div class="sub">{loading}</div> }.into_any(),
        LoadState::Failed(text) => view! {
            <div class="note" style="color:var(--danger)"><span>{text.clone()}</span></div>
        }
        .into_any(),
        LoadState::Ready(_) => ().into_any(),
    }
}

/// Show a note in the Notes tab.
pub fn open_note(ctx: BrainCtx, id: NoteId) {
    ctx.tab.set(BrainTab::Notes);
    ctx.open_note.set(Some(id));
}

/// Fill one note from `GET /api/brain/notes/:id`.
pub fn load_note(ctx: BrainCtx, id: NoteId, detail: RwSignal<LoadState<NoteDetail>>) {
    spawn_local(async move {
        match api::get::<NoteDetail>(&format!("/api/brain/notes/{id}")).await {
            Ok(note) => detail.set(LoadState::Ready(note)),
            Err(failure) => fail_load(ctx, detail, &failure),
        }
    });
}

/// Start a care run now. The daemon answers with the number of proposals it made, and `reload`
/// runs after the toast so the open lists show them.
pub fn run_care(ui: Ui, missing: RwSignal<bool>, reload: impl FnOnce() + 'static) {
    spawn_local(async move {
        match api::post_empty::<serde_json::Value>("/api/brain/care/run").await {
            Ok(reply) => {
                let count = reply
                    .get("proposals")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                ui.show_toast(care_done_text(count), ToastKind::Ok);
                reload();
            }
            Err(failure) => report_failure(ui, missing, &failure),
        }
    });
}

/// Set the Brain mode and wait for the daemon to take it.
pub async fn post_brain_mode(mode: BrainMode) -> Result<serde_json::Value, ApiFailure> {
    api::post::<_, serde_json::Value>("/api/brain/mode", &SetBrainMode { mode }).await
}

/// Set the Brain mode from a button, then refresh the snapshot so the header shows it.
pub fn set_brain_mode(ui: Ui, mode: BrainMode) {
    spawn_local(async move {
        settle(ui, post_brain_mode(mode).await);
    });
}

/// Build the notes from the project. Only learn mode may write, so the Brain switches to learn
/// first. The button stays disabled until the seed answers.
pub fn build_notes(ctx: BrainCtx) {
    if ctx.seeding.get_untracked() {
        return;
    }
    ctx.seeding.set(true);
    let mode = ctx
        .ui
        .snapshot
        .with_untracked(|snapshot| snapshot.brain_mode);
    spawn_local(async move {
        let switched = mode_needs_switch(mode);
        if switched {
            if let Err(failure) = post_brain_mode(BrainMode::Learn).await {
                report_failure(ctx.ui, ctx.missing, &failure);
                ctx.seeding.set(false);
                return;
            }
            state::refresh(ctx.ui);
        }
        match api::post_empty::<SeedReport>("/api/brain/seed").await {
            Ok(report) => {
                ctx.ui
                    .show_toast(seed_done_text(switched, report.created), ToastKind::Ok);
                bump(ctx);
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
        ctx.seeding.set(false);
    });
}

fn load_header(ctx: BrainCtx) {
    spawn_local(async move {
        match api::get::<NoteList>("/api/brain/notes").await {
            Ok(list) => {
                ctx.note_count.set(list.notes.len());
                ctx.vault_path.set(list.vault_path);
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
    spawn_local(async move {
        match api::get::<Vec<ProposalView>>("/api/brain/proposals").await {
            Ok(proposals) => ctx.open_proposals.set(proposals.len()),
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
    spawn_local(async move {
        match api::get::<GraphData>("/api/brain/graph").await {
            Ok(graph) => ctx.graph.set(LoadState::Ready(graph)),
            Err(failure) => fail_load(ctx, ctx.graph, &failure),
        }
    });
}

/// Copy the vault path so the user can open it in Obsidian.
fn copy_vault_path(ctx: BrainCtx) {
    let path = ctx.vault_path.get_untracked();
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            return;
        };
        let promise = window.navigator().clipboard().write_text(&path);
        match JsFuture::from(promise).await {
            Ok(_) => ctx.ui.show_toast(
                "Vault path copied. Open it as a vault in Obsidian.",
                ToastKind::Ok,
            ),
            Err(_) => ctx
                .ui
                .show_toast("Could not copy the vault path.", ToastKind::Danger),
        }
    });
}

#[component]
pub fn BrainPanel() -> impl IntoView {
    let ctx = BrainCtx::new(use_ui());
    Effect::new(move |_| {
        ctx.revision.track();
        load_header(ctx);
    });
    view! {
        {move || if ctx.missing.get() {
            unavailable_note().into_any()
        } else {
            view! {
                <PanelHeader ctx=ctx />
                <PanelTabs ctx=ctx />
                <PanelBody ctx=ctx />
            }.into_any()
        }}
    }
}

/// The note that replaces a Brain view the daemon cannot answer.
pub fn unavailable_note() -> impl IntoView {
    view! {
        <div class="note">
            <Icon name="info" size=14 />
            <span>{UNAVAILABLE}</span>
        </div>
    }
}

#[component]
fn PanelHeader(ctx: BrainCtx) -> impl IntoView {
    let mode = move || ctx.ui.snapshot.with(|snapshot| snapshot.brain_mode);
    view! {
        <div class="card" style="margin-bottom:.8em">
            <div class="row sp wrap">
                <b class="row"><Icon name="brain" size=16 />" Brain"</b>
                <span class="chip">{move || mode_name(mode())}</span>
            </div>
            <div class="sub" style="margin:.3em 0">{move || mode_sentence(mode())}</div>
            <div class="row wrap">
                <span class="chip">{move || format!("{} notes", ctx.note_count.get())}</span>
                <span class="chip mono">{move || ctx.vault_path.get()}</span>
                <button class="btn sm" on:click=move |_| copy_vault_path(ctx)>
                    "Open in Obsidian"
                </button>
                {move || (!ctx.ui.on_phone() && mode_needs_switch(mode())).then(|| view! {
                    <button class="btn sm" on:click=move |_| set_brain_mode(ctx.ui, BrainMode::Learn)>
                        {SWITCH_TO_LEARN_LABEL}
                    </button>
                })}
            </div>
        </div>
    }
}

#[component]
fn PanelTabs(ctx: BrainCtx) -> impl IntoView {
    view! {
        <div class="seg" style="margin-bottom:.8em">
            {BrainTab::ALL
                .into_iter()
                .map(|tab| tab_button(ctx, tab))
                .collect_view()}
        </div>
    }
}

fn tab_button(ctx: BrainCtx, tab: BrainTab) -> impl IntoView {
    let label = move || match tab {
        BrainTab::Proposals => proposals_tab_label(ctx.open_proposals.get()),
        BrainTab::Notes | BrainTab::Graph => tab.label().to_owned(),
    };
    view! {
        <button aria-pressed=move || (ctx.tab.get() == tab).to_string()
            on:click=move |_| ctx.tab.set(tab)>
            {label}
        </button>
    }
}

#[component]
fn PanelBody(ctx: BrainCtx) -> impl IntoView {
    move || match ctx.tab.get() {
        BrainTab::Notes => view! { <NotesTab ctx=ctx /> }.into_any(),
        BrainTab::Graph => view! { <GraphTab ctx=ctx /> }.into_any(),
        BrainTab::Proposals => view! { <ProposalsTab ctx=ctx /> }.into_any(),
    }
}
