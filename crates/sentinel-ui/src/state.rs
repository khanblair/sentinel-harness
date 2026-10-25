//! The page state: the last snapshot from the daemon, the live connection, and what the user has
//! open. Components read signals from `Ui`. They never talk to the daemon directly except through
//! `api`, and every answer comes back as a new snapshot or an event.

use std::collections::HashMap;

use leptos::prelude::*;
use sentinel_core::api::{ProjectInfo, WhoAmI};
use sentinel_core::chat::ChatChange;
use sentinel_core::routines::{Brief, Routine};
use sentinel_core::{Attachment, ChatId, Event, EventRecord, Intent, Snapshot, ToolId};

use crate::chats::RenameState;
use crate::components::add_project::{ProjectDialog, carry_out_wish};
use crate::components::confirm_clear::ClearAction;
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
    Settings {
        page: String,
    },
    Guide,
    Palette,
    BypassConfirm,
    /// Asks before a chat and its uploads are removed.
    ConfirmDelete {
        chat: ChatId,
    },
    /// Asks before a data reset runs. Each Clear button in Settings opens it.
    ConfirmClear {
        title: String,
        body: String,
        action: ClearAction,
    },
    AddFiles,
    AddFolder,
    AddBacklog,
    AddNote,
    /// Use a skill: one skill for the next message.
    AddSkill,
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
    /// The chat the stream shows. `None` until the first snapshot picks one.
    pub open_chat: RwSignal<Option<ChatId>>,
    pub modal: RwSignal<Option<Modal>>,
    /// Pending attachments of the open chat. They go with the next message.
    pub attachments: RwSignal<Vec<Attachment>>,
    /// The type the next message carries, set by a bare type command such as `/ask`.
    pub next_intent: RwSignal<Option<Intent>>,
    /// The chat row whose title is being edited in place.
    pub renaming: RwSignal<Option<RenameState>>,
    pub toast: RwSignal<Option<Toast>>,
    pub prefs: RwSignal<Prefs>,
    pub terminal: RwSignal<HashMap<ToolId, Vec<String>>>,
    pub popover: RwSignal<Option<Popover>>,
    /// True once the first snapshot has arrived, so the page does not guess before it knows.
    pub loaded: RwSignal<bool>,
    /// The setup wizard was opened on purpose, for example from a menu.
    pub setup_open: RwSignal<bool>,
    /// The routines as `GET /api/routines` last returned them. The sidebar counts them.
    pub routines: RwSignal<Vec<Routine>>,
    /// The latest briefs, newest first, as `GET /api/briefs` last returned them.
    pub briefs: RwSignal<Vec<Brief>>,
    /// True when a paired phone is calling, from `GET /api/whoami`. Off until that answers, so
    /// the computer's own page is never locked while it loads.
    pub from_phone: RwSignal<bool>,
    /// Sidebar width in pixels as the drag handle left it. The layout clamps it on render.
    pub sidebar_width: RwSignal<u32>,
    /// Right panel width in pixels as the drag handle left it. The layout clamps it on render.
    pub panel_width: RwSignal<u32>,
    /// Page width in pixels. The side widths are fitted to it so the stream keeps its room.
    pub viewport: RwSignal<u32>,
    /// The projects as `GET /api/projects` last returned them, the primary first.
    pub projects: RwSignal<Vec<ProjectInfo>>,
    /// The Add project or Remove project dialog, when one is open.
    pub project_dialog: RwSignal<Option<ProjectDialog>>,
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
    /// The menu of one chat row, opened from its more button or a long press.
    Chat,
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

/// The tabs open at the start: Board and Reserved on desktop, none elsewhere.
fn default_tabs(layout: Layout) -> Vec<PanelId> {
    if layout.panel_open_by_default() {
        vec![PanelId::Board, PanelId::Reserved]
    } else {
        Vec::new()
    }
}

impl Ui {
    pub fn new(layout: Layout, prefs: Prefs) -> Self {
        let (sidebar_width, panel_width) = (prefs.sidebar_w, prefs.panel_w);
        Self {
            snapshot: RwSignal::new(Snapshot::empty()),
            connection: RwSignal::new(Connection::Connecting),
            layout: RwSignal::new(layout),
            sidebar_open: RwSignal::new(layout.sidebar_open_by_default()),
            panel_open: RwSignal::new(layout.panel_open_by_default()),
            panel_wide: RwSignal::new(false),
            panel_tabs: RwSignal::new(default_tabs(layout)),
            active_panel: RwSignal::new(default_tabs(layout).first().copied()),
            focus: RwSignal::new(None),
            target: RwSignal::new(Target::Sentinel),
            open_chat: RwSignal::new(None),
            modal: RwSignal::new(None),
            attachments: RwSignal::new(Vec::new()),
            next_intent: RwSignal::new(None),
            renaming: RwSignal::new(None),
            toast: RwSignal::new(None),
            prefs: RwSignal::new(prefs),
            terminal: RwSignal::new(HashMap::new()),
            popover: RwSignal::new(None),
            loaded: RwSignal::new(false),
            setup_open: RwSignal::new(false),
            routines: RwSignal::new(Vec::new()),
            briefs: RwSignal::new(Vec::new()),
            from_phone: RwSignal::new(false),
            sidebar_width: RwSignal::new(sidebar_width),
            panel_width: RwSignal::new(panel_width),
            viewport: RwSignal::new(0),
            projects: RwSignal::new(Vec::new()),
            project_dialog: RwSignal::new(None),
        }
    }

    /// The sidebar edge can be dragged: desktop with the sidebar open.
    pub fn sidebar_resizable(&self) -> bool {
        self.layout.get() == Layout::Desktop && self.sidebar_open.get()
    }

    /// The panel edge can be dragged: desktop with the panel open and not expanded.
    pub fn panel_resizable(&self) -> bool {
        self.layout.get() == Layout::Desktop && self.panel_open.get() && !self.panel_wide.get()
    }

    /// True when a paired phone is calling. Rights checks use this, never the layout.
    pub fn on_phone(&self) -> bool {
        self.from_phone.get()
    }

    /// Replace the snapshot with a fresh one from the daemon. The first one also asks who is
    /// calling.
    pub fn apply_snapshot(&self, snapshot: Snapshot) {
        let first = !self.loaded.get_untracked();
        self.snapshot.set(snapshot);
        self.loaded.set(true);
        if first {
            self.load_caller();
            carry_out_wish(*self);
        }
    }

    /// Ask the daemon who is calling. A failed call leaves `from_phone` false.
    fn load_caller(&self) {
        let ui = *self;
        leptos::task::spawn_local(async move {
            if let Ok(who) = crate::api::get::<WhoAmI>("/api/whoami").await {
                ui.from_phone.set(!who.from_pc);
            }
        });
    }

    /// Show a chat in the stream. On phone the sidebar drawer closes.
    pub fn open(&self, id: ChatId) {
        self.open_chat.set(Some(id));
        if self.layout.get_untracked() == Layout::Phone {
            self.sidebar_open.set(false);
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

    /// Add a live event. Agent output joins the terminal buffer. Every event moves the last
    /// event id on, and only events of the open chat join the stream.
    pub fn ingest(&self, record: EventRecord) {
        if let Event::AgentOutput { agent, line, .. } = &record.event {
            self.terminal.update(|map| {
                let lines = map.entry(agent.clone()).or_default();
                lines.push(line.clone());
                let extra = lines.len().saturating_sub(TERMINAL_LINES);
                lines.drain(..extra);
            });
        }
        self.drop_deleted_open_chat(&record.event);
        let open = self.open_chat.get_untracked();
        let shown = belongs_to(record.chat.as_ref(), open.as_ref());
        self.snapshot.update(|snapshot| {
            snapshot.last_event_id = snapshot.last_event_id.max(record.id);
            if shown {
                snapshot.events.push(record);
                let extra = snapshot.events.len().saturating_sub(STREAM_EVENTS);
                snapshot.events.drain(..extra);
            }
        });
    }

    /// A deleted chat cannot stay open. Clearing it makes the next reload pick another chat.
    fn drop_deleted_open_chat(&self, event: &Event) {
        if let Event::ChatChanged {
            chat,
            change: ChatChange::Deleted,
        } = event
        {
            self.open_chat.update(|open| {
                if open.as_ref() == Some(chat) {
                    *open = None;
                }
            });
        }
    }
}

/// Whether an event shows in the stream of the open chat. An event with no chat shows everywhere.
pub fn belongs_to(record_chat: Option<&ChatId>, open: Option<&ChatId>) -> bool {
    record_chat.is_none_or(|chat| open == Some(chat))
}

pub fn provide_ui(ui: Ui) {
    provide_context(ui);
}

pub fn use_ui() -> Ui {
    expect_context::<Ui>()
}

/// Fetch a fresh snapshot of the open chat and replace the current one. Call this after any change
/// sent to the daemon, and when the socket reconnects.
pub fn refresh(ui: Ui) {
    crate::ws::reload(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> ChatId {
        ChatId::new(text)
    }

    #[test]
    fn an_event_with_no_chat_shows_in_every_stream() {
        assert!(belongs_to(None, Some(&id("c1"))));
        assert!(belongs_to(None, None));
    }

    #[test]
    fn an_event_of_the_open_chat_shows() {
        assert!(belongs_to(Some(&id("c1")), Some(&id("c1"))));
    }

    #[test]
    fn an_event_of_another_chat_stays_out_of_the_stream() {
        assert!(!belongs_to(Some(&id("c1")), Some(&id("c2"))));
        assert!(!belongs_to(Some(&id("c1")), None));
    }
}
