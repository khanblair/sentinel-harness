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

/// Desktop widths in pixels. The drag handles keep them inside these bounds.
pub const SIDEBAR_MIN: u32 = 200;
pub const SIDEBAR_MAX: u32 = 480;
pub const SIDEBAR_DEFAULT: u32 = 264;
pub const PANEL_MIN: u32 = 280;
pub const PANEL_MAX: u32 = 720;
pub const PANEL_DEFAULT: u32 = 440;
/// The stream never gets narrower than this while both sides are open.
pub const STREAM_MIN: u32 = 360;

/// Fits both sides into the window. The panel gives way first, then the sidebar. Neither goes
/// below its minimum, so a very narrow window can still leave the stream under `STREAM_MIN`.
pub fn clamp_widths(window: u32, sidebar: u32, panel: u32) -> (u32, u32) {
    let sidebar = sidebar.clamp(SIDEBAR_MIN, SIDEBAR_MAX);
    let panel = panel.clamp(PANEL_MIN, PANEL_MAX);
    let room = window.saturating_sub(STREAM_MIN);
    let panel = panel.min(room.saturating_sub(sidebar).max(PANEL_MIN));
    let sidebar = sidebar.min(room.saturating_sub(panel).max(SIDEBAR_MIN));
    (sidebar, panel)
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
    Context,
}

impl PanelId {
    pub const ALL: [PanelId; 13] = [
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
        PanelId::Context,
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
            PanelId::Context => "Context",
        }
    }
}

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
    fn top_bar_has_the_six_documented_panels() {
        let labels: Vec<_> = PanelId::TOP_BAR.iter().map(|p| p.label()).collect();
        assert_eq!(
            labels,
            [
                "Board", "Terminal", "Changes", "Reserved", "Brain", "Remote"
            ]
        );
    }
}

#[cfg(test)]
mod width_tests {
    use super::*;

    #[test]
    fn widths_that_fit_the_window_stay_as_they_are() {
        assert_eq!(clamp_widths(1600, 264, 440), (264, 440));
    }

    #[test]
    fn widths_outside_their_range_snap_to_the_bounds() {
        assert_eq!(clamp_widths(1600, 50, 900), (SIDEBAR_MIN, PANEL_MAX));
        assert_eq!(clamp_widths(1600, 900, 50), (SIDEBAR_MAX, PANEL_MIN));
    }

    #[test]
    fn the_panel_gives_way_before_the_sidebar_on_a_narrow_window() {
        assert_eq!(clamp_widths(1000, 264, 440), (264, 376));
        assert_eq!(clamp_widths(900, 264, 440), (260, PANEL_MIN));
    }

    #[test]
    fn the_stream_keeps_its_minimum_when_the_window_allows_it() {
        let (sidebar, panel) = clamp_widths(1000, 480, 720);
        assert!(1000 - sidebar - panel >= STREAM_MIN, "{sidebar} {panel}");
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
