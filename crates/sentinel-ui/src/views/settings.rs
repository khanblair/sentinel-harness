//! The Settings modal (`docs/ui-rules.md`, Views: Settings). The overlay frame is
//! `components/modal.rs`. This file fills it: the header, the page nav, and the page body. The
//! open page lives in `ui.modal`, so the nav and the body always agree.

use leptos::prelude::*;

use crate::components::budget_form::AgentsPage;
use crate::components::budget_form::BudgetPage;
use crate::components::connection_card::{ConnectionsPage, DevicesPage};
use crate::components::icon::Icon;
use crate::state::{Modal, Ui, use_ui};
use crate::views::appearance::AppearancePage;
use crate::views::routines_page::RoutinesPage;
use crate::views::settings_pages::{BrainPage, GeneralPage};
use crate::views::skills_page::SkillsPage;
use crate::views::telegram_slot::{HostCardSlot, TelegramSettingsSlot};

/// One row of the settings nav: its id as the modal stores it, its icon, and its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsPage {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
}

/// The thirteen pages, in the prototype's order.
pub static SET_PAGES: [SettingsPage; 13] = [
    SettingsPage {
        id: "general",
        icon: "gear",
        label: "General",
    },
    SettingsPage {
        id: "appearance",
        icon: "palette",
        label: "Appearance",
    },
    SettingsPage {
        id: "agents",
        icon: "agent",
        label: "Agents and tools",
    },
    SettingsPage {
        id: "connections",
        icon: "net",
        label: "Connections",
    },
    SettingsPage {
        id: "integrations",
        icon: "link",
        label: "Integrations",
    },
    SettingsPage {
        id: "perms",
        icon: "shield",
        label: "Permissions",
    },
    SettingsPage {
        id: "skills",
        icon: "book",
        label: "Skills",
    },
    SettingsPage {
        id: "routines",
        icon: "clock",
        label: "Routines",
    },
    SettingsPage {
        id: "brain",
        icon: "brain",
        label: "Brain",
    },
    SettingsPage {
        id: "budget",
        icon: "dollar",
        label: "Budget",
    },
    SettingsPage {
        id: "devices",
        icon: "phone",
        label: "Devices",
    },
    SettingsPage {
        id: "shortcuts",
        icon: "keyboard",
        label: "Shortcuts",
    },
    SettingsPage {
        id: "about",
        icon: "info",
        label: "About",
    },
];

/// One labeled row of a settings page: the label and hint on the left, the control on the right.
/// `modifier` adds a class such as `col` for rows that stack their control below the label.
#[component]
pub fn Field(
    #[prop(into)] label: String,
    #[prop(optional, into)] hint: String,
    #[prop(optional)] modifier: &'static str,
    children: Children,
) -> impl IntoView {
    let class = format!("fld {modifier}");
    let hint_view = (!hint.is_empty()).then(|| view! { <span>{hint}</span> });
    view! {
        <div class=class>
            <div class="l">
                <b>{label}</b>
                {hint_view}
            </div>
            {children()}
        </div>
    }
}

/// Other ids that open a settings page. The Agents panel's "Manage in settings" link sends `tools`.
const PAGE_ALIASES: [(&str, &str); 1] = [("tools", "agents")];

/// The page for a stored id. An unknown id opens General, so a stale link never shows an
/// empty body.
pub fn page_from_id(id: &str) -> &'static SettingsPage {
    let id = PAGE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == id)
        .map_or(id, |(_, target)| *target);
    SET_PAGES
        .iter()
        .find(|page| page.id == id)
        .unwrap_or(&SET_PAGES[0])
}

/// The id of the page the modal shows. Any modal other than Settings shows General.
pub fn active_page_id(modal: Option<&Modal>) -> &'static str {
    match modal {
        Some(Modal::Settings { page }) => page_from_id(page).id,
        Some(
            Modal::Guide
            | Modal::Palette
            | Modal::BypassConfirm
            | Modal::ConfirmDelete { .. }
            | Modal::ConfirmClear { .. }
            | Modal::AddFiles
            | Modal::AddFolder
            | Modal::AddBacklog
            | Modal::AddNote
            | Modal::AddSkill,
        )
        | None => SET_PAGES[0].id,
    }
}

/// The dialog's content: the header, then the nav and the body side by side (or stacked on a
/// phone). Put it inside the modal frame, which already draws the `.modal` box.
#[component]
pub fn SettingsDialog() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="modal-hd">
            <Icon name="gear" size=18 />
            <h3>"Settings"</h3>
            <button class="ibtn" aria-label="Close settings" on:click=move |_| ui.modal.set(None)>
                <Icon name="x" size=17 />
            </button>
        </div>
        <div class="modal-body">
            <div class="set">
                <SettingsNav />
                {move || {
                    let page = active_page_id(ui.modal.get().as_ref()).to_owned();
                    view! { <SettingsBody page=page /> }
                }}
            </div>
        </div>
    }
}

#[component]
pub fn SettingsNav() -> impl IntoView {
    let ui = use_ui();
    view! {
        <nav class="set-nav" id="setNav">
            {SET_PAGES
                .into_iter()
                .map(|page| nav_button(ui, page))
                .collect_view()}
        </nav>
    }
}

fn nav_button(ui: Ui, page: SettingsPage) -> impl IntoView {
    let current = move || (active_page_id(ui.modal.get().as_ref()) == page.id).to_string();
    view! {
        <button
            aria-current=current
            on:click=move |_| ui.modal.set(Some(Modal::Settings { page: page.id.to_owned() }))
        >
            <Icon name=page.icon size=16 />
            <span>{page.label}</span>
        </button>
    }
}

/// The body of one page. An unknown id shows General.
#[component]
pub fn SettingsBody(page: String) -> impl IntoView {
    let id = page_from_id(&page).id;
    view! {
        <div class="set-body" id="setBody" data-page=page>
            {page_body(id)}
        </div>
    }
}

fn page_body(id: &str) -> AnyView {
    match id {
        "appearance" => view! { <AppearancePage /> }.into_any(),
        "agents" => view! { <AgentsPage /> }.into_any(),
        "connections" => view! { <ConnectionsPage /> }.into_any(),
        "integrations" => view! { <IntegrationsPage /> }.into_any(),
        "perms" => view! { <PermissionsPage /> }.into_any(),
        "skills" => view! { <SkillsPage /> }.into_any(),
        "routines" => view! { <RoutinesPage /> }.into_any(),
        "brain" => view! { <BrainPage /> }.into_any(),
        "budget" => view! { <BudgetPage /> }.into_any(),
        "devices" => view! { <DevicesPage /> }.into_any(),
        "shortcuts" => view! { <ShortcutsPage /> }.into_any(),
        "about" => view! { <AboutPage /> }.into_any(),
        _ => view! { <GeneralPage /> }.into_any(),
    }
}

#[component]
fn IntegrationsPage() -> impl IntoView {
    view! {
        <h2>"Integrations"</h2>
        <p class="lead">"Other places Sentinel can reach you."</p>
        <HostCardSlot />
        <TelegramSettingsSlot />
    }
}

#[component]
fn PermissionsPage() -> impl IntoView {
    view! {
        <h2>"Permissions"</h2>
        <p class="lead">"What agents may do without asking."</p>
        <div class="note" style="margin-bottom:.8em">
            <Icon name="lock" size=14 />
            <span>"Agents can only write files their task has reserved. No setting turns this off."</span>
        </div>
        <div class="note">
            <Icon name="info" size=14 />
            <span>"Pick a permission mode in the Permissions panel."</span>
        </div>
    }
}

const SHORTCUTS: [(&str, &str); 8] = [
    ("New chat", "Ctrl or Cmd + Shift + O"),
    ("Command palette", "Ctrl or Cmd + K"),
    ("Toggle sidebar", "Ctrl or Cmd + B"),
    ("Toggle panels", "Ctrl or Cmd + J"),
    ("Settings", "Ctrl or Cmd + ,"),
    ("Send message", "Enter"),
    ("New line", "Shift + Enter"),
    ("Commands", "Type /"),
];

#[component]
fn ShortcutsPage() -> impl IntoView {
    view! {
        <h2>"Shortcuts"</h2>
        <p class="lead">"Keyboard, and gestures on a phone."</p>
        <table class="dl">
            <tr><th>"Action"</th><th>"Keys"</th></tr>
            {SHORTCUTS
                .into_iter()
                .map(|(action, keys)| view! { <tr><td>{action}</td><td class="mono">{keys}</td></tr> })
                .collect_view()}
        </table>
        <p class="lead" style="margin-top:1em">
            "Phone: tap a run to open it. Use the bottom tabs for Board, Reserved and Changes. More opens every other panel."
        </p>
    }
}

/// The paths of the docs an operator reads first. They are paths, not links, because the docs
/// are not served from the daemon.
const DOC_PATHS: [&str; 5] = [
    "docs/product-scope.md",
    "docs/architecture.md",
    "docs/ui-rules.md",
    "docs/ui-tokens.md",
    "docs/code-standards.md",
];

#[component]
fn AboutPage() -> impl IntoView {
    view! {
        <h2>"About"</h2>
        <p class="lead">"Sentinel runs coding agents in one shared git tree. A task counts as verified only after a separate check passes."</p>
        <Field label="Version">
            <span class="k">{env!("CARGO_PKG_VERSION")}</span>
        </Field>
        <div class="psec">
            <h5>"Docs"</h5>
            <ul>
                {DOC_PATHS
                    .into_iter()
                    .map(|path| view! { <li><span class="k">{path}</span></li> })
                    .collect_view()}
            </ul>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_keep_the_prototype_order_and_labels() {
        let labels: Vec<&str> = SET_PAGES.iter().map(|page| page.label).collect();
        assert_eq!(
            labels,
            [
                "General",
                "Appearance",
                "Agents and tools",
                "Connections",
                "Integrations",
                "Permissions",
                "Skills",
                "Routines",
                "Brain",
                "Budget",
                "Devices",
                "Shortcuts",
                "About",
            ]
        );
    }

    #[test]
    fn every_page_id_round_trips_through_page_from_id() {
        for page in SET_PAGES {
            assert_eq!(page_from_id(page.id), &page);
        }
    }

    #[test]
    fn the_agents_panel_link_opens_the_agents_page() {
        assert_eq!(page_from_id("tools").id, "agents");
        assert_eq!(page_from_id("agents").id, "agents");
    }

    #[test]
    fn an_unknown_page_id_opens_general() {
        assert_eq!(page_from_id("nope").id, "general");
        assert_eq!(page_from_id("").label, "General");
    }

    #[test]
    fn every_page_icon_is_in_the_icon_set() {
        for page in SET_PAGES {
            assert!(!crate::icons::markup(page.icon).is_empty(), "{}", page.icon);
        }
    }

    #[test]
    fn the_active_page_follows_the_open_settings_modal() {
        let brain = Modal::Settings {
            page: "brain".to_owned(),
        };
        assert_eq!(active_page_id(Some(&brain)), "brain");
        assert_eq!(active_page_id(Some(&Modal::Guide)), "general");
        assert_eq!(active_page_id(None), "general");
    }

    #[test]
    fn an_unknown_page_in_the_modal_opens_general() {
        let stale = Modal::Settings {
            page: "gone".to_owned(),
        };
        assert_eq!(active_page_id(Some(&stale)), "general");
    }
}
