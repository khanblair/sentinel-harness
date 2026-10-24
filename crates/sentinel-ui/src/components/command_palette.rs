//! `CommandPalette`: Ctrl or Cmd + K. It draws its own overlay (`.pal`), not the modal frame,
//! because the palette has no dialog chrome. It is open while `Modal::Palette` is set.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::RunRequest;
use web_sys::KeyboardEvent;

use crate::api;
use crate::components::icon::Icon;
use crate::components::sidebar::new_chat;
use crate::components::top_bar::settle;
use crate::palette::{
    PaletteAction, PaletteItem, filter_items, next_theme, palette_items, step_highlight,
};
use crate::state::{Modal, Target, Ui, use_ui};
use crate::tour_state;

#[component]
pub fn CommandPalette() -> impl IntoView {
    let ui = use_ui();
    move || (ui.modal.get() == Some(Modal::Palette)).then(|| view! { <PaletteBox /> })
}

#[component]
fn PaletteBox() -> impl IntoView {
    let ui = use_ui();
    let query = RwSignal::new(String::new());
    let highlight = RwSignal::new(0_usize);
    let input = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| {
        if let Some(field) = input.get() {
            let _ = field.focus();
        }
    });
    view! {
        <div class="pal" id="ov-pal" on:click=move |_| ui.modal.set(None)>
            <div role="dialog" aria-label="Command palette" on:click=move |event| event.stop_propagation()>
                <input id="palIn" node_ref=input placeholder="Type a command" aria-label="Command"
                    prop:value=move || query.get()
                    on:input=move |event| {
                        query.set(event_target_value(&event));
                        highlight.set(0);
                    }
                    on:keydown=move |event: KeyboardEvent| on_palette_key(ui, query, highlight, &event) />
                <div class="lst" id="palList">
                    {move || palette_rows(ui, query, highlight)}
                </div>
            </div>
        </div>
    }
}

/// The rows for the current query. An empty result says so.
fn palette_rows(ui: Ui, query: RwSignal<String>, highlight: RwSignal<usize>) -> AnyView {
    let items = visible_items(ui, &query.get());
    if items.is_empty() {
        return view! { <div class="note" style="padding:.8em">"No match."</div> }.into_any();
    }
    let current = highlight.get();
    items
        .into_iter()
        .enumerate()
        .map(|(position, item)| palette_row(ui, item, position == current))
        .collect_view()
        .into_any()
}

/// One command. The highlighted row gets the hover background, since the stylesheet has no
/// rule for a keyboard selection.
fn palette_row(ui: Ui, item: PaletteItem, selected: bool) -> impl IntoView {
    let style = if selected {
        "background:var(--surface2)"
    } else {
        ""
    };
    let action = item.action;
    view! {
        <button class="mi" style=style on:click=move |_| run_item(ui, action.clone())>
            <Icon name=item.icon size=16 />
            <span>{item.label}</span>
        </button>
    }
}

fn visible_items(ui: Ui, query: &str) -> Vec<PaletteItem> {
    let items = ui
        .snapshot
        .with_untracked(|snapshot| palette_items(&snapshot.tools));
    filter_items(&items, query).into_iter().cloned().collect()
}

fn on_palette_key(
    ui: Ui,
    query: RwSignal<String>,
    highlight: RwSignal<usize>,
    event: &KeyboardEvent,
) {
    let items = visible_items(ui, &query.get_untracked());
    let key = event.key();
    match key.as_str() {
        "ArrowDown" | "ArrowUp" => {
            event.prevent_default();
            let forward = key == "ArrowDown";
            let next = step_highlight(highlight.get_untracked(), items.len(), forward);
            highlight.set(next);
        }
        "Enter" => {
            event.prevent_default();
            if let Some(item) = items.get(highlight.get_untracked()) {
                run_item(ui, item.action.clone());
            }
        }
        _ => {}
    }
}

/// Closes the palette, then runs the command.
fn run_item(ui: Ui, action: PaletteAction) {
    ui.modal.set(None);
    match action {
        PaletteAction::SwitchAgent(agent) => ui.target.set(Target::Agent(agent)),
        PaletteAction::FocusAgent(agent) => ui.focus.set(Some(agent)),
        PaletteAction::OpenPanel(panel) => ui.open_panel(panel),
        PaletteAction::StartRun => start_run(ui),
        PaletteAction::NewChat => new_chat(ui),
        PaletteAction::ToggleTheme => {
            ui.prefs
                .update(|prefs| prefs.theme = next_theme(prefs.theme));
        }
        PaletteAction::OpenSettings => ui.modal.set(Some(Modal::Settings {
            page: "general".to_owned(),
        })),
        PaletteAction::OpenGuide => ui.modal.set(Some(Modal::Guide)),
        PaletteAction::StartTour => tour_state::start(ui),
    }
}

/// Starts the whole backlog of the project in the open chat, as the Start run button does.
fn start_run(ui: Ui) {
    let request = RunRequest {
        chat: ui.open_chat.get_untracked(),
        ..RunRequest::default()
    };
    spawn_local(async move {
        settle(ui, api::start_run(&request).await);
    });
}
