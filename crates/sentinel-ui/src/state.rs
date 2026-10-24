//! The page state: the last snapshot from the daemon, the live connection, and what the user has
//! open. Components read signals from `Ui`. They never talk to the daemon directly except through
//! `api`, and every answer comes back as a new snapshot or an event.

use std::collections::HashMap;

use leptos::prelude::*;
use sentinel_core::{Event, EventRecord, Snapshot, ToolId};

use crate::layout::{Layout, PanelId};
use crate::prefs::Prefs;

/// Output lines kept per agent for the Terminal panel.
const TERMINAL_LINES: usize = 500;
/// Events kept for the stream.
const STREAM_EVENTS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    Connecting,
    Live,
    /// Disconnected. Shows the banner "Disconnected. Showing last state from HH:MM".
    Offline {
        since: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    Settings { page: String },
    Guide,
    Palette,
    BypassConfirm,
}

/// A short result message, shown about three seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Ok,
    Warn,
    Danger,
    Info,
}

impl ToastKind {
    pub fn class(self) -> &'static str {
        match self {
            ToastKind::Ok => "ok",
            ToastKind::Warn => "warn",
            ToastKind::Danger => "danger",
            ToastKind::Info => "info",
        }
    }
}

/// Where the composer sends a message: to Sentinel, or straight to one agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Sentinel,
    Agent(ToolId),
}

#[derive(Clone, Copy)]
pub struct Ui {
    pub snapshot: RwSignal<Snapshot>,
    pub connection: RwSignal<Connection>,
    pub layout: RwSignal<Layout>,
    pub sidebar_open: RwSignal<bool>,
    pub panel_open: RwSignal<bool>,
    pub panel_wide: RwSignal<bool>,
    pub panel_tabs: RwSignal<Vec<PanelId>>,
    pub active_panel: RwSignal<Option<PanelId>>,
    /// Show only this agent in the stream.
    pub focus: RwSignal<Option<ToolId>>,
    pub target: RwSignal<Target>,
    pub modal: RwSignal<Option<Modal>>,
    pub toast: RwSignal<Option<Toast>>,
    pub prefs: RwSignal<Prefs>,
    pub terminal: RwSignal<HashMap<ToolId, Vec<String>>>,
    pub popover: RwSignal<Option<Popover>>,
}

/// Which menu a popover shows. Each kind has one body in `components/menus.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopoverKind {
    Title,
    Agents,
    Permission,
    Brain,
    Target,
    More,
    Panel,
    Attach,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// An open menu, pinned under the element with `anchor_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Popover {
    pub anchor_id: String,
    pub align: Align,
    pub kind: PopoverKind,
}

impl Ui {
    pub fn new(layout: Layout, prefs: Prefs) -> Self {
        Self {
            snapshot: RwSignal::new(Snapshot::empty()),
            connection: RwSignal::new(Connection::Connecting),
            layout: RwSignal::new(layout),
            sidebar_open: RwSignal::new(layout.sidebar_open_by_default()),
            panel_open: RwSignal::new(false),
            panel_wide: RwSignal::new(false),
            panel_tabs: RwSignal::new(Vec::new()),
            active_panel: RwSignal::new(None),
            focus: RwSignal::new(None),
            target: RwSignal::new(Target::Sentinel),
            modal: RwSignal::new(None),
            toast: RwSignal::new(None),
            prefs: RwSignal::new(prefs),
            terminal: RwSignal::new(HashMap::new()),
            popover: RwSignal::new(None),
        }
    }

    /// Open a panel as a tab and show it.
    pub fn open_panel(&self, panel: PanelId) {
        self.panel_tabs.update(|tabs| {
            if !tabs.contains(&panel) {
                tabs.push(panel);
            }
        });
        self.active_panel.set(Some(panel));
        self.panel_open.set(true);
    }

    pub fn close_panel_tab(&self, panel: PanelId) {
        self.panel_tabs.update(|tabs| tabs.retain(|p| *p != panel));
        let remaining = self.panel_tabs.get_untracked();
        self.active_panel.set(remaining.last().copied());
        if remaining.is_empty() {
            self.panel_open.set(false);
        }
    }

    pub fn show_toast(&self, text: impl Into<String>, kind: ToastKind) {
        self.toast.set(Some(Toast {
            text: text.into(),
            kind,
        }));
    }

    /// Add a live event: it joins the stream list, and agent output joins the terminal buffer.
    pub fn ingest(&self, record: EventRecord) {
        if let Event::AgentOutput { agent, line, .. } = &record.event {
            self.terminal.update(|map| {
                let lines = map.entry(agent.clone()).or_default();
                lines.push(line.clone());
                let extra = lines.len().saturating_sub(TERMINAL_LINES);
                lines.drain(..extra);
            });
        }
        self.snapshot.update(|snapshot| {
            snapshot.last_event_id = snapshot.last_event_id.max(record.id);
            snapshot.events.push(record);
            let extra = snapshot.events.len().saturating_sub(STREAM_EVENTS);
            snapshot.events.drain(..extra);
        });
    }
}

pub fn provide_ui(ui: Ui) {
    provide_context(ui);
}

pub fn use_ui() -> Ui {
    expect_context::<Ui>()
}

/// Fetch a fresh snapshot and replace the current one. Call this after any change you sent to
/// the daemon, and when the socket reconnects.
pub fn refresh(ui: Ui) {
    leptos::task::spawn_local(async move {
        match crate::api::snapshot().await {
            Ok(snapshot) => ui.snapshot.set(snapshot),
            Err(error) => ui.show_toast(error.to_string(), ToastKind::Danger),
        }
    });
}
