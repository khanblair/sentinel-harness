//! Command palette items and the search over them. The overlay is `components/command_palette.rs`.

use sentinel_core::{ToolId, ToolInfo, ToolStatus};

use crate::components::panel_host::panel_icon;
use crate::layout::PanelId;
use crate::prefs::Theme;

/// The palette shows at most this many matches.
pub const MAX_RESULTS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    SwitchAgent(ToolId),
    FocusAgent(ToolId),
    OpenPanel(PanelId),
    StartRun,
    ToggleTheme,
    OpenSettings,
    OpenGuide,
    StartTour,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteItem {
    pub label: String,
    pub icon: &'static str,
    pub action: PaletteAction,
}

impl PaletteItem {
    fn new(label: String, icon: &'static str, action: PaletteAction) -> Self {
        Self {
            label,
            icon,
            action,
        }
    }
}

/// Agents that can take work now, then every panel, then the page commands.
pub fn palette_items(tools: &[ToolInfo]) -> Vec<PaletteItem> {
    let mut items: Vec<PaletteItem> = tools
        .iter()
        .filter(|tool| tool.enabled && matches!(tool.status, ToolStatus::Ready))
        .flat_map(agent_items)
        .collect();
    items.extend(PanelId::ALL.into_iter().map(panel_item));
    items.extend(page_items());
    items
}

fn agent_items(tool: &ToolInfo) -> [PaletteItem; 2] {
    [
        PaletteItem::new(
            format!("Switch to {}", tool.name),
            "agent",
            PaletteAction::SwitchAgent(tool.id.clone()),
        ),
        PaletteItem::new(
            format!("Focus stream on {}", tool.name),
            "eye",
            PaletteAction::FocusAgent(tool.id.clone()),
        ),
    ]
}

fn panel_item(panel: PanelId) -> PaletteItem {
    PaletteItem::new(
        format!("Open {}", panel.label()),
        panel_icon(panel),
        PaletteAction::OpenPanel(panel),
    )
}

fn page_items() -> [PaletteItem; 5] {
    [
        PaletteItem::new("Start a run".to_owned(), "play2", PaletteAction::StartRun),
        PaletteItem::new("Toggle theme".to_owned(), "sun", PaletteAction::ToggleTheme),
        PaletteItem::new(
            "Open settings".to_owned(),
            "gear",
            PaletteAction::OpenSettings,
        ),
        PaletteItem::new(
            "How to use Sentinel".to_owned(),
            "book",
            PaletteAction::OpenGuide,
        ),
        PaletteItem::new(
            "Take the tour".to_owned(),
            "play2",
            PaletteAction::StartTour,
        ),
    ]
}

/// Words in any order must all appear in the label, ignoring case. A label that starts with
/// the whole query comes first. Equal ranks keep the order they came in.
pub fn filter_items<'a>(items: &'a [PaletteItem], query: &str) -> Vec<&'a PaletteItem> {
    let phrase = query.trim().to_lowercase();
    let words: Vec<&str> = phrase.split_whitespace().collect();
    let mut matches: Vec<(bool, &PaletteItem)> = items
        .iter()
        .filter_map(|item| rank(item, &words, &phrase))
        .collect();
    matches.sort_by_key(|(not_prefix, _)| *not_prefix);
    matches
        .into_iter()
        .map(|(_, item)| item)
        .take(MAX_RESULTS)
        .collect()
}

/// `Some(false)` for a prefix match, `Some(true)` for other matches, `None` for no match.
fn rank<'a>(
    item: &'a PaletteItem,
    words: &[&str],
    phrase: &str,
) -> Option<(bool, &'a PaletteItem)> {
    let label = item.label.to_lowercase();
    if !words.iter().all(|word| label.contains(word)) {
        return None;
    }
    Some((!label.starts_with(phrase), item))
}

/// Up or down through the matches, wrapping at both ends. Nothing to move through gives 0.
pub fn step_highlight(current: usize, count: usize, forward: bool) -> usize {
    if count == 0 {
        return 0;
    }
    if forward {
        (current + 1) % count
    } else {
        (current + count - 1) % count
    }
}

/// The palette's theme command flips between light and dark. System counts as light.
pub fn next_theme(current: Theme) -> Theme {
    match current {
        Theme::Dark => Theme::Light,
        Theme::System | Theme::Light => Theme::Dark,
    }
}

#[cfg(test)]
mod tests {
    use sentinel_core::Tier;

    use super::*;

    fn tool(id: &str, name: &str, status: ToolStatus, enabled: bool) -> ToolInfo {
        ToolInfo {
            id: ToolId::new(id),
            name: name.to_owned(),
            path: String::new(),
            version: String::new(),
            adapter: String::new(),
            status,
            tier: Tier::Smart,
            enabled,
        }
    }

    fn labels(items: &[&PaletteItem]) -> Vec<String> {
        items.iter().map(|item| item.label.clone()).collect()
    }

    fn item(label: &str) -> PaletteItem {
        PaletteItem::new(label.to_owned(), "book", PaletteAction::OpenGuide)
    }

    #[test]
    fn only_enabled_ready_agents_get_palette_items() {
        let tools = [
            tool("claude", "Claude Code", ToolStatus::Ready, true),
            tool("codex", "Codex", ToolStatus::Ready, false),
            tool("aider", "Aider", ToolStatus::Missing, true),
        ];
        let items = palette_items(&tools);
        assert!(items.iter().any(|i| i.label == "Switch to Claude Code"));
        assert!(
            items
                .iter()
                .any(|i| i.label == "Focus stream on Claude Code")
        );
        assert!(!items.iter().any(|i| i.label.contains("Codex")));
        assert!(!items.iter().any(|i| i.label.contains("Aider")));
    }

    #[test]
    fn every_panel_gets_an_open_item() {
        let items = palette_items(&[]);
        let open_count = items
            .iter()
            .filter(|i| matches!(i.action, PaletteAction::OpenPanel(_)))
            .count();
        assert_eq!(open_count, PanelId::ALL.len());
        assert!(items.iter().any(|i| i.label == "Open Board"));
    }

    #[test]
    fn page_commands_are_listed_last_without_preview_sizes() {
        let items = palette_items(&[]);
        assert!(items.iter().any(|i| i.label == "Start a run"));
        assert!(items.iter().any(|i| i.label == "Take the tour"));
        assert!(!items.iter().any(|i| i.label.starts_with("Preview")));
    }

    #[test]
    fn filter_matches_any_order_and_ignores_case() {
        let items = [item("Open Cost"), item("Open Board"), item("Take the tour")];
        let found = filter_items(&items, "COST open");
        assert_eq!(labels(&found), vec!["Open Cost"]);
    }

    #[test]
    fn filter_puts_prefix_matches_first() {
        let items = [item("Focus stream on Board"), item("Board tools")];
        let found = filter_items(&items, "board");
        assert_eq!(labels(&found), vec!["Board tools", "Focus stream on Board"]);
    }

    #[test]
    fn empty_query_lists_items_in_their_order() {
        let items = [item("B one"), item("A two")];
        assert_eq!(labels(&filter_items(&items, "  ")), vec!["B one", "A two"]);
    }

    #[test]
    fn filter_stops_at_the_result_cap() {
        let items: Vec<PaletteItem> = (0..20).map(|n| item(&format!("Item {n}"))).collect();
        assert_eq!(filter_items(&items, "item").len(), MAX_RESULTS);
    }

    #[test]
    fn highlight_wraps_at_both_ends() {
        assert_eq!(step_highlight(2, 3, true), 0);
        assert_eq!(step_highlight(0, 3, false), 2);
        assert_eq!(step_highlight(0, 0, true), 0);
    }

    #[test]
    fn theme_command_flips_between_light_and_dark() {
        assert_eq!(next_theme(Theme::Dark), Theme::Light);
        assert_eq!(next_theme(Theme::Light), Theme::Dark);
        assert_eq!(next_theme(Theme::System), Theme::Dark);
    }
}
