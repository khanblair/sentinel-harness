//! The tour's seven stops, their copy, and the math that places a coach mark. Reading the DOM
//! and driving the timers lives in `tour_state.rs`.

/// Space between an anchor and its coach mark, in px.
const GAP: f64 = 12.0;
/// The coach mark keeps this far from the edge of the app, in px.
const EDGE: f64 = 8.0;
/// Pulls the Beside card down so it lines up with the run list header.
const BESIDE_DROP: f64 = 60.0;
/// A horizontal touch past this many px counts as a swipe.
const SWIPE_MIN: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

/// Where the coach mark sits compared with its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Above,
    Below,
    Beside,
    Centered,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub x: f64,
    pub y: f64,
    pub side: Side,
}

/// How a stop places its coach mark. `Beside` only applies on desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    Auto,
    Beside,
    Center,
}

/// A step the user asks for, by key or by swipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Next,
    Back,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TourStop {
    /// Selectors tried in order. The first one that is on screen wins.
    pub anchors: &'static [&'static str],
    pub title: &'static str,
    pub body: &'static str,
    /// The stop needs the sidebar drawer open on phone.
    pub opens_sidebar: bool,
    pub pin: Pin,
}

pub const STOPS: &[TourStop] = &[
    TourStop {
        anchors: &["#sbList", ".sb-scroll"],
        title: "Your runs",
        body: "Runs are grouped by repository. A blue dot means a run needs you. A phone icon means you can steer it from a phone.",
        opens_sidebar: true,
        pin: Pin::Beside,
    },
    TourStop {
        anchors: &["#agentStack"],
        title: "Switch agents",
        body: "These avatars are the agents in this run. Click one to focus the stream on it, open its terminal, or kill it.",
        opens_sidebar: false,
        pin: Pin::Auto,
    },
    TourStop {
        anchors: &["#hdIcons", ".hd-icons", "#tabbar"],
        title: "Panels slide in",
        body: "Board, terminal, changes, reserved files, brain and phone open in a side panel. More holds files, cost, report, agents and permissions.",
        opens_sidebar: false,
        pin: Pin::Auto,
    },
    TourStop {
        anchors: &["#stream", ".stream"],
        title: "The stream",
        body: "Everything that happens, in order: plans, reserved files, agent steps, checkpoints, tests and verdicts. It scrolls like a chat.",
        opens_sidebar: false,
        pin: Pin::Center,
    },
    TourStop {
        anchors: &["#permChip"],
        title: "Permissions",
        body: "Choose how much agents may do without asking. Agents can only write files their task has reserved, whatever you pick.",
        opens_sidebar: false,
        pin: Pin::Auto,
    },
    TourStop {
        anchors: &["#targetChip"],
        title: "Send to",
        body: "Messages go to Sentinel, which routes by tier, or straight to one agent. Type / for commands such as /pause and /cap 1.50.",
        opens_sidebar: false,
        pin: Pin::Auto,
    },
    TourStop {
        anchors: &[".sb-ft .ibtn:last-child"],
        title: "Settings",
        body: "Appearance, agents, connections, permissions, budget and devices live here. Fonts, sizes and colors change instantly.",
        opens_sidebar: true,
        pin: Pin::Auto,
    },
];

/// The stop after `step`, or `None` after the last one.
pub fn next_stop(step: usize, total: usize) -> Option<usize> {
    let next = step + 1;
    (next < total).then_some(next)
}

pub fn previous_stop(step: usize) -> usize {
    step.saturating_sub(1)
}

/// Keys that move the tour: arrows go back and forward, Escape ends it.
pub fn move_for_key(key: &str) -> Option<Move> {
    match key {
        "ArrowRight" | "ArrowDown" => Some(Move::Next),
        "ArrowLeft" | "ArrowUp" => Some(Move::Back),
        "Escape" => Some(Move::End),
        _ => None,
    }
}

/// A swipe to the left goes forward. A short or small touch does nothing.
pub fn move_for_swipe(delta_x: f64) -> Option<Move> {
    if delta_x <= -SWIPE_MIN {
        Some(Move::Next)
    } else if delta_x >= SWIPE_MIN {
        Some(Move::Back)
    } else {
        None
    }
}

/// The desktop run list card sits beside its anchor. Every other layout uses the auto rules.
pub fn effective_pin(pin: Pin, desktop: bool) -> Pin {
    match pin {
        Pin::Beside if !desktop => Pin::Auto,
        Pin::Auto | Pin::Beside | Pin::Center => pin,
    }
}

pub fn clamp(value: f64, low: f64, high: f64) -> f64 {
    value.max(low).min(high)
}

/// Places a coach mark for the given pin. The viewport is the app, not the device.
pub fn place(pin: Pin, anchor: Rect, card: Size, viewport: Size) -> Placement {
    match pin {
        Pin::Auto => placement(anchor, card, viewport),
        Pin::Beside => Placement {
            x: anchor.x + anchor.w + GAP,
            y: anchor.y + BESIDE_DROP,
            side: Side::Beside,
        },
        Pin::Center => Placement {
            x: (viewport.w - card.w) / 2.0,
            y: (viewport.h - card.h) / 2.0,
            side: Side::Centered,
        },
    }
}

/// Below the anchor when there is room, above it near the bottom edge. A tall anchor in the top
/// half keeps the card below it, so the card never covers the thing it points at.
pub fn placement(anchor: Rect, card: Size, viewport: Size) -> Placement {
    let (y, side) = vertical(anchor, card, viewport);
    let x = clamp(anchor.x, EDGE, viewport.w - card.w - EDGE);
    Placement { x, y, side }
}

fn vertical(anchor: Rect, card: Size, viewport: Size) -> (f64, Side) {
    let below = anchor.y + anchor.h + GAP;
    if below + card.h <= viewport.h - EDGE {
        return below_or_clamped(anchor, card, viewport, below);
    }
    let above = (anchor.y - card.h - GAP).max(EDGE);
    (above, Side::Above)
}

fn below_or_clamped(anchor: Rect, card: Size, viewport: Size, below: f64) -> (f64, Side) {
    let top_half = anchor.y < viewport.h / 2.0;
    let wide = anchor.w > viewport.w * 0.6;
    if below < EDGE || (top_half && wide) {
        let clamped = clamp(below, EDGE, viewport.h - card.h - EDGE);
        return (clamped, Side::Below);
    }
    (below, Side::Below)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Size = Size { w: 400.0, h: 800.0 };
    const CARD: Size = Size { w: 200.0, h: 100.0 };

    fn anchor_at(x: f64, y: f64) -> Rect {
        Rect {
            x,
            y,
            w: 80.0,
            h: 30.0,
        }
    }

    #[test]
    fn a_small_anchor_in_the_middle_puts_the_card_below_it() {
        let placed = placement(anchor_at(100.0, 300.0), CARD, VIEWPORT);
        assert_eq!(placed.side, Side::Below);
        assert!((placed.y - 342.0).abs() < 1e-9);
    }

    #[test]
    fn an_anchor_near_the_bottom_puts_the_card_above_it() {
        let placed = placement(anchor_at(100.0, 720.0), CARD, VIEWPORT);
        assert_eq!(placed.side, Side::Above);
        assert!((placed.y - 608.0).abs() < 1e-9);
    }

    #[test]
    fn the_card_never_leaves_the_right_edge() {
        let placed = placement(anchor_at(380.0, 300.0), CARD, VIEWPORT);
        assert!((placed.x - 192.0).abs() < 1e-9);
    }

    #[test]
    fn the_card_never_leaves_the_left_edge() {
        let placed = placement(anchor_at(-50.0, 300.0), CARD, VIEWPORT);
        assert!((placed.x - EDGE).abs() < 1e-9);
    }

    #[test]
    fn a_wide_anchor_in_the_top_half_keeps_the_card_below_it() {
        let wide = Rect {
            x: 0.0,
            y: 20.0,
            w: 300.0,
            h: 30.0,
        };
        let placed = placement(wide, CARD, VIEWPORT);
        assert_eq!(placed.side, Side::Below);
    }

    #[test]
    fn a_center_pin_puts_the_card_in_the_middle_of_the_viewport() {
        let placed = place(Pin::Center, anchor_at(0.0, 0.0), CARD, VIEWPORT);
        assert_eq!(
            placed,
            Placement {
                x: 100.0,
                y: 350.0,
                side: Side::Centered
            }
        );
    }

    #[test]
    fn a_beside_pin_puts_the_card_to_the_right_of_its_anchor() {
        let placed = place(Pin::Beside, anchor_at(10.0, 40.0), CARD, VIEWPORT);
        assert_eq!(
            placed,
            Placement {
                x: 102.0,
                y: 100.0,
                side: Side::Beside
            }
        );
    }

    #[test]
    fn beside_is_used_only_on_desktop() {
        assert_eq!(effective_pin(Pin::Beside, false), Pin::Auto);
        assert_eq!(effective_pin(Pin::Beside, true), Pin::Beside);
        assert_eq!(effective_pin(Pin::Center, false), Pin::Center);
    }

    #[test]
    fn next_stop_stops_after_the_last_stop() {
        assert_eq!(next_stop(0, 7), Some(1));
        assert_eq!(next_stop(5, 7), Some(6));
        assert_eq!(next_stop(6, 7), None);
    }

    #[test]
    fn previous_stop_stops_at_the_first_stop() {
        assert_eq!(previous_stop(0), 0);
        assert_eq!(previous_stop(3), 2);
    }

    #[test]
    fn arrows_and_escape_move_the_tour() {
        assert_eq!(move_for_key("ArrowRight"), Some(Move::Next));
        assert_eq!(move_for_key("ArrowUp"), Some(Move::Back));
        assert_eq!(move_for_key("Escape"), Some(Move::End));
        assert_eq!(move_for_key("a"), None);
    }

    #[test]
    fn only_a_long_horizontal_swipe_moves_the_tour() {
        assert_eq!(move_for_swipe(-120.0), Some(Move::Next));
        assert_eq!(move_for_swipe(120.0), Some(Move::Back));
        assert_eq!(move_for_swipe(-10.0), None);
    }

    #[test]
    fn the_tour_has_seven_stops_with_anchors_and_copy() {
        assert_eq!(STOPS.len(), 7);
        for stop in STOPS {
            assert!(!stop.anchors.is_empty(), "{} has no anchor", stop.title);
            assert!(!stop.body.is_empty(), "{} has no body", stop.title);
        }
    }

    #[test]
    fn the_stream_stop_is_centered_and_the_run_list_stop_is_beside() {
        assert_eq!(STOPS[3].title, "The stream");
        assert_eq!(STOPS[3].pin, Pin::Center);
        assert_eq!(STOPS[0].pin, Pin::Beside);
    }
}
