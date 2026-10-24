//! The page. Mounts the workspace shell, applies the prefs, and wires the window listeners
//! (`docs/ui-rules.md`, Layout and Responsive Layouts; `docs/mobile.md`, Offline Behavior).

use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use sentinel_core::{ToolId, ToolInfo};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::components::command_palette::CommandPalette;
use crate::components::connection_banner::ConnectionBanner;
use crate::components::icon::Icon;
use crate::components::modal::ModalHost;
use crate::components::panel_host::PanelHost;
use crate::components::popover::{PopoverHost, install_outside_click};
use crate::components::sidebar::Sidebar;
use crate::components::tab_bar::{TabBar, touch_ended, touch_started};
use crate::components::toast::Toast;
use crate::components::top_bar::TopBar;
use crate::components::tour_coach::TourCoach;
use crate::layout::Layout;
use crate::prefs::Prefs;
use crate::state::{Modal, Ui, provide_ui, use_ui};
use crate::theme;
use crate::tour_state::provide_tour;
use crate::views::composer::Composer;
use crate::views::onboarding::SetupWizard;
use crate::views::stream::StreamView;
use crate::ws;

const PREFS_KEY: &str = "sentinel.prefs";
/// Used when the window width cannot be read. Desktop is the widest layout.
const FALLBACK_WIDTH: f64 = 1280.0;

/// A keyboard shortcut that the page handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    Sidebar,
    Panels,
    Palette,
}

pub fn shortcut_for(key: &str) -> Option<Shortcut> {
    match key.to_ascii_lowercase().as_str() {
        "b" => Some(Shortcut::Sidebar),
        "j" => Some(Shortcut::Panels),
        "k" => Some(Shortcut::Palette),
        _ => None,
    }
}

/// The name shown for an agent in the focus bar. An unknown id shows as its raw text.
pub fn agent_label(tools: &[ToolInfo], agent: &ToolId) -> String {
    tools
        .iter()
        .find(|tool| tool.id == *agent)
        .map_or_else(|| agent.as_str().to_owned(), |tool| tool.name.clone())
}

fn window_width() -> f64 {
    web_sys::window()
        .and_then(|window| window.inner_width().ok())
        .and_then(|width| width.as_f64())
        .unwrap_or(FALLBACK_WIDTH)
}

pub(crate) fn load_prefs() -> Prefs {
    LocalStorage::get::<Prefs>(PREFS_KEY)
        .unwrap_or_default()
        .clamped()
}

/// Moves to a new layout and sets the sidebar and panels the way the prototype does.
fn change_layout(ui: Ui, layout: Layout) {
    if ui.layout.get_untracked() == layout {
        return;
    }
    ui.layout.set(layout);
    match layout {
        Layout::Desktop => {
            ui.sidebar_open.set(true);
            ui.panel_open
                .set(!ui.panel_tabs.with_untracked(Vec::is_empty));
        }
        Layout::Tablet => {
            ui.sidebar_open.set(true);
            ui.panel_open.set(false);
        }
        Layout::Phone => {
            ui.sidebar_open.set(false);
            ui.panel_open.set(false);
        }
    }
}

fn run_shortcut(ui: Ui, shortcut: Shortcut) {
    match shortcut {
        Shortcut::Sidebar => ui.sidebar_open.update(|open| *open = !*open),
        Shortcut::Panels => ui.panel_open.update(|open| *open = !*open),
        Shortcut::Palette => ui.modal.set(Some(Modal::Palette)),
    }
}

/// Escape closes the menu first, then the modal.
fn close_top(ui: Ui) {
    if ui.popover.get_untracked().is_some() {
        ui.popover.set(None);
    } else {
        ui.modal.set(None);
    }
}

fn handle_key(ui: Ui, event: &web_sys::KeyboardEvent) {
    let key = event.key();
    if event.ctrl_key() || event.meta_key() {
        if let Some(shortcut) = shortcut_for(&key) {
            event.prevent_default();
            run_shortcut(ui, shortcut);
        }
    } else if key == "Escape" {
        close_top(ui);
    }
}

/// The scrim closes the drawer and the panels on phone and tablet.
fn close_overlays(ui: Ui) {
    if ui.layout.get_untracked() == Layout::Phone {
        ui.sidebar_open.set(false);
    }
    ui.panel_open.set(false);
    ui.popover.set(None);
}

fn watch_window(ui: Ui) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let on_resize = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        change_layout(ui, Layout::from_width(window_width()));
    });
    let _ = window.add_event_listener_with_callback("resize", on_resize.as_ref().unchecked_ref());
    on_resize.forget();
    let on_key =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
            handle_key(ui, &event);
        });
    let _ = window.add_event_listener_with_callback("keydown", on_key.as_ref().unchecked_ref());
    on_key.forget();
}

/// One-time setup after mount: listeners, the first snapshot, and the socket.
fn install(ui: Ui) {
    watch_window(ui);
    theme::follow_system(ui);
    install_outside_click(ui);
    ws::reload(ui);
    ws::connect(ui);
}

#[component]
pub fn App() -> impl IntoView {
    let ui = Ui::new(Layout::from_width(window_width()), load_prefs());
    provide_ui(ui);
    provide_tour();
    let touch_start = RwSignal::new(None::<f64>);
    Effect::new(move |_| install(ui));
    Effect::new(move |_| {
        let prefs = ui.prefs.get();
        theme::apply(&prefs);
        // A blocked store (private window) keeps the prefs for this visit only.
        let _ = LocalStorage::set(PREFS_KEY, &prefs);
    });
    view! {
        <div class="app" id="app"
            class:sb-open=move || ui.sidebar_open.get()
            class:rp-open=move || ui.panel_open.get()
            class:rp-wide=move || ui.panel_wide.get()
            data-layout=move || ui.layout.get().attr()>
            <Sidebar />
            <TopBar />
            <main class="main" id="main"
                on:touchstart=move |event| touch_started(ui, touch_start, &event)
                on:touchend=move |event| touch_ended(ui, touch_start, &event)>
                <ConnectionBanner />
                <FocusBar />
                <div class="stream"><div class="stream-in" id="streamIn"><StreamView /></div></div>
                <div class="comp-wrap" id="compWrap"><Composer /></div>
            </main>
            <PanelHost />
            <TabBar />
            <div class="scrim" id="scrim" on:click=move |_| close_overlays(ui)></div>
            <Toast />
            <PopoverHost />
            <SetupGate />
            <ModalHost />
            <CommandPalette />
            <TourCoach />
        </div>
    }
}

/// The setup wizard covers the app until setup is done, and whenever someone opens it.
#[component]
fn SetupGate() -> impl IntoView {
    let ui = use_ui();
    let visible = move || {
        ui.setup_open.get() || (ui.loaded.get() && !ui.snapshot.with(|s| s.onboarding.complete))
    };
    view! {
        <Show when=visible>
            <div class="wiz" id="wiz" role="dialog" aria-label="Sentinel setup">
                <SetupWizard />
            </div>
        </Show>
    }
}

#[component]
fn FocusBar() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="focusbar" class:on=move || ui.focus.get().is_some()>
            {move || ui.focus.get().map(|agent| {
                let name = ui.snapshot.with(|snapshot| agent_label(&snapshot.tools, &agent));
                view! {
                    <Icon name="eye" size=15 />
                    <span>"Showing only " <b>{name}</b> " and Sentinel events"</span>
                    <span style="flex:1"></span>
                    <button class="btn sm" on:click=move |_| ui.focus.set(None)>"Show all agents"</button>
                }
            })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_or_cmd_letters_map_to_the_three_shortcuts() {
        assert_eq!(shortcut_for("b"), Some(Shortcut::Sidebar));
        assert_eq!(shortcut_for("J"), Some(Shortcut::Panels));
        assert_eq!(shortcut_for("k"), Some(Shortcut::Palette));
        assert_eq!(shortcut_for("x"), None);
    }

    #[test]
    fn focus_bar_names_the_agent_it_shows() {
        let tool = ToolInfo {
            id: ToolId::new("claude"),
            name: "Claude Code".to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: String::new(),
            status: sentinel_core::ToolStatus::Ready,
            tier: sentinel_core::Tier::Smart,
            enabled: true,
        };
        assert_eq!(agent_label(&[tool], &ToolId::new("claude")), "Claude Code");
        assert_eq!(agent_label(&[], &ToolId::new("codex")), "codex");
    }
}
