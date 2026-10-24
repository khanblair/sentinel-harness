//! Breakpoints and panel names. The layout follows the width of the app, not the device type
//! (`docs/ui-tokens.md`, Breakpoints).

/// Under 620 px is phone, 620 to 1179 px is tablet, 1180 px and up is desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Phone,
    Tablet,
    Desktop,
}

impl Layout {
    pub fn from_width(width: f64) -> Self {
        if width >= 1180.0 {
            Layout::Desktop
        } else if width >= 620.0 {
            Layout::Tablet
        } else {
            Layout::Phone
        }
    }

    /// The value of `data-layout` on the app root.
    pub fn attr(self) -> &'static str {
        match self {
            Layout::Phone => "phone",
            Layout::Tablet => "tablet",
            Layout::Desktop => "desktop",
        }
    }

    /// The sidebar is open by default on tablet and desktop. On phone it is a drawer.
    pub fn sidebar_open_by_default(self) -> bool {
        self != Layout::Phone
    }

    /// The right panel starts open with the Board and Reserved tabs on desktop only.
    pub fn panel_open_by_default(self) -> bool {
        self == Layout::Desktop
    }
}

/// A panel that slides in beside the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelId {
    Board,
    Terminal,
    Changes,
    Reserved,
    Brain,
    Remote,
    Files,
    Cost,
    Report,
    Agents,
    Permissions,
    Telegram,
}

impl PanelId {
    pub const ALL: [PanelId; 12] = [
        PanelId::Board,
        PanelId::Terminal,
        PanelId::Changes,
        PanelId::Reserved,
        PanelId::Brain,
        PanelId::Remote,
        PanelId::Files,
        PanelId::Cost,
        PanelId::Report,
        PanelId::Agents,
        PanelId::Permissions,
        PanelId::Telegram,
    ];

    /// The six panels that have an icon in the top bar. The rest open from the More menu.
    pub const TOP_BAR: [PanelId; 6] = [
        PanelId::Board,
        PanelId::Terminal,
        PanelId::Changes,
        PanelId::Reserved,
        PanelId::Brain,
        PanelId::Remote,
    ];

    pub fn id(self) -> &'static str {
        match self {
            PanelId::Board => "board",
            PanelId::Terminal => "terminal",
            PanelId::Changes => "changes",
            PanelId::Reserved => "reserved",
            PanelId::Brain => "brain",
            PanelId::Remote => "remote",
            PanelId::Files => "files",
            PanelId::Cost => "cost",
            PanelId::Report => "report",
            PanelId::Agents => "agents",
            PanelId::Permissions => "permissions",
            PanelId::Telegram => "telegram",
        }
    }

    pub fn from_id(id: &str) -> Option<PanelId> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }

    /// The tab title.
    pub fn label(self) -> &'static str {
        match self {
            PanelId::Board => "Board",
            PanelId::Terminal => "Terminal",
            PanelId::Changes => "Changes",
            PanelId::Reserved => "Reserved",
            PanelId::Brain => "Brain",
            PanelId::Remote => "Remote",
            PanelId::Files => "Files",
            PanelId::Cost => "Cost",
            PanelId::Report => "Report",
            PanelId::Agents => "Agents",
            PanelId::Permissions => "Permissions",
            PanelId::Telegram => "Telegram",
        }
    }
}

/// The five phone tabs. `More` opens every other panel and settings.
pub const PHONE_TABS: [&str; 5] = ["stream", "board", "reserved", "changes", "more"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_match_the_token_doc() {
        assert_eq!(Layout::from_width(375.0), Layout::Phone);
        assert_eq!(Layout::from_width(619.0), Layout::Phone);
        assert_eq!(Layout::from_width(620.0), Layout::Tablet);
        assert_eq!(Layout::from_width(1179.0), Layout::Tablet);
        assert_eq!(Layout::from_width(1180.0), Layout::Desktop);
    }

    #[test]
    fn sidebar_is_open_by_default_above_phone_width() {
        assert!(!Layout::Phone.sidebar_open_by_default());
        assert!(Layout::Tablet.sidebar_open_by_default());
        assert!(Layout::Desktop.sidebar_open_by_default());
    }

    #[test]
    fn every_panel_round_trips_through_its_id() {
        for panel in PanelId::ALL {
            assert_eq!(PanelId::from_id(panel.id()), Some(panel));
        }
        assert_eq!(PanelId::from_id("nope"), None);
    }

    #[test]
    fn top_bar_has_the_six_documented_panels() {
        let ids: Vec<_> = PanelId::TOP_BAR.iter().map(|p| p.id()).collect();
        assert_eq!(
            ids,
            [
                "board", "terminal", "changes", "reserved", "brain", "remote"
            ]
        );
    }
}

#[cfg(test)]
mod panel_default_tests {
    use super::Layout;

    #[test]
    fn only_desktop_opens_the_panel_at_the_start() {
        assert!(Layout::Desktop.panel_open_by_default());
        assert!(!Layout::Tablet.panel_open_by_default());
        assert!(!Layout::Phone.panel_open_by_default());
    }
}
