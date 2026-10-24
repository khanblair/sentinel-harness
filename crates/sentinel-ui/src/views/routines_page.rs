//! The Routines page of Settings (`docs/product-scope.md`, F25): the routines, the add button, the
//! latest briefs, and the notes on missed times, Telegram, and privacy.

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::brief_card::BriefCard;
use crate::components::icon::Icon;
use crate::components::routine_editor::{EditorMode, RoutineEditor};
use crate::components::routine_row::RoutineRow;
use crate::routines::{self, BriefNotice, latest_brief_ready};
use crate::state::{ToastKind, Ui, use_ui};

/// The notes under "Good to know": a label and its text.
const NOTES: [(&str, &str); 3] = [
    (
        "Missed times",
        "A routine missed while Sentinel was off still runs at start when it is under 2 hours late. Older ones are skipped.",
    ),
    (
        "Telegram",
        "Needs the Reports level. /brief and /wrapup send them on demand.",
    ),
    (
        "Privacy",
        "A brief holds counts, task ids, and reasons. In hackathon mode it holds counts only. It never holds code, files, keys, or note text.",
    ),
];

#[component]
pub fn RoutinesPage() -> impl IntoView {
    let ui = use_ui();
    let editor = RwSignal::new(None::<EditorMode>);
    spawn_local(async move { routines::load_page(ui).await });
    announce_new_briefs(ui);
    view! {
        <h2>"Routines"</h2>
        <p class="lead">"Jobs that run at a set time on this computer's clock. A brief is built from the facts of your project. It uses no agent and costs nothing. The scheduler checks every 30 seconds while Sentinel is on."</p>
        {move || {
            ui.routines
                .get()
                .into_iter()
                .map(|routine| view! { <RoutineRow routine editor /> })
                .collect_view()
        }}
        <AddRow editor />
        <BriefsSection />
        <GoodToKnow />
        {move || editor.get().map(|mode| view! { <EditorOverlay mode editor /> })}
    }
}

/// A brief that arrives while the page is open reloads the list and shows a toast. The first run
/// does not announce: briefs that were already there stay quiet.
fn announce_new_briefs(ui: Ui) {
    let notice = Memo::new(move |_| {
        ui.snapshot
            .with(|snapshot| latest_brief_ready(&snapshot.events))
    });
    Effect::new(move |previous: Option<Option<BriefNotice>>| {
        let current = notice.get();
        if let (Some(_), Some(found)) = (previous, &current) {
            ui.show_toast(
                format!("{} is ready, in Routines", found.title),
                ToastKind::Ok,
            );
            spawn_local(async move { routines::load_briefs(ui).await });
        }
        current
    });
}

/// The overlay frame for the editor. The Settings modal stays under it, as its own body does.
#[component]
fn EditorOverlay(mode: EditorMode, editor: RwSignal<Option<EditorMode>>) -> impl IntoView {
    view! {
        <div class="ov" id="ov-routine" on:click=move |_| editor.set(None)>
            <div class="modal sm" role="dialog" on:click=move |event| event.stop_propagation()>
                <RoutineEditor mode on_close=Callback::new(move |()| editor.set(None)) />
            </div>
        </div>
    }
}

/// "Add a reminder". On a phone it is off and says why.
#[component]
fn AddRow(editor: RwSignal<Option<EditorMode>>) -> impl IntoView {
    let ui = use_ui();
    let phone = move || ui.on_phone();
    view! {
        <div class="row wrap" style="margin:.8em 0">
            <button class="btn" disabled=phone on:click=move |_| editor.set(Some(EditorMode::Add))>
                <Icon name="plus" size=14 />
                " Add a reminder"
            </button>
            <Show when=phone>
                <span class="sub">"Needs the computer"</span>
            </Show>
        </div>
    }
}

#[component]
fn BriefsSection() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="psec">
            <h5>"Latest briefs"</h5>
            {move || {
                let briefs = ui.briefs.get();
                if briefs.is_empty() {
                    view! {
                        <div class="note">
                            <Icon name="info" size=14 />
                            <span>"No briefs yet. Press Send now on a routine to see one."</span>
                        </div>
                    }
                    .into_any()
                } else {
                    briefs
                        .into_iter()
                        .map(|brief| view! { <BriefCard brief /> })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn GoodToKnow() -> impl IntoView {
    view! {
        <div class="psec">
            <h5>"Good to know"</h5>
            {NOTES
                .into_iter()
                .map(|(label, hint)| {
                    view! {
                        <div class="fld">
                            <div class="l"><b>{label}</b><span>{hint}</span></div>
                        </div>
                    }
                })
                .collect_view()}
        </div>
    }
}
