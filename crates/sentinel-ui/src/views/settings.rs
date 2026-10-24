//! The Settings modal's page list and the page dispatch (`docs/ui-rules.md`, Views: Settings).
//! The overlay frame is `components/modal.rs`. This file fills it: the nav and the body.

use leptos::prelude::*;

/// One row of the settings nav: its id as the modal stores it, its icon, and its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsPage {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
}

/// The eleven pages, in the prototype's order.
pub static SET_PAGES: [SettingsPage; 11] = [
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

/// The page for a stored id. An unknown id opens General, so a stale link never shows an
/// empty body.
pub fn page_from_id(id: &str) -> &'static SettingsPage {
    SET_PAGES
        .iter()
        .find(|page| page.id == id)
        .unwrap_or(&SET_PAGES[0])
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
}
