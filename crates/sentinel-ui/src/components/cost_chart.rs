//! `CostChart`: spend over time as an SVG line, the cap as a dashed line, and a text table as the
//! alternative for screen readers and copy.

use leptos::prelude::*;
use sentinel_core::Cost;

/// The drawing box, in SVG units. The chart is scaled to the full width of the panel.
const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 60.0;
/// Keeps the line off the top and bottom edges.
const PAD: f64 = 2.0;
/// Local offset for the EAT clock (UTC+3), in seconds.
const EAT_OFFSET_SECONDS: i64 = 3 * 3600;

/// `part / whole` in thousandths, clamped to 0 to 1000. Integer math keeps the float casts exact.
fn permille(part: u128, whole: u128) -> u32 {
    if whole == 0 {
        return 0;
    }
    u32::try_from((part * 1000 / whole).min(1000)).unwrap_or(1000)
}

/// The top of the vertical scale: the cap, or the biggest spend when that is higher.
pub fn chart_top(points: &[(i64, u64)], cap: u64) -> u64 {
    points
        .iter()
        .map(|(_, micro)| *micro)
        .chain(std::iter::once(cap))
        .max()
        .unwrap_or(1)
        .max(1)
}

/// SVG `y` for a value on the scale. Zero sits at the bottom, the top sits near the top edge.
fn y_for(value: u64, top: u64, height: f64) -> f64 {
    let ratio = permille(u128::from(value), u128::from(top));
    height - PAD - f64::from(ratio) * (height - 2.0 * PAD) / 1000.0
}

/// The `points` attribute of the spend line. Time runs along x from the first to the last point.
pub fn polyline_points(points: &[(i64, u64)], width: f64, height: f64, cap: u64) -> String {
    let Some(&(first, _)) = points.first() else {
        return String::new();
    };
    let last = points.last().map_or(first, |point| point.0);
    let span = u128::try_from(last - first).unwrap_or(0);
    let top = chart_top(points, cap);
    points
        .iter()
        .map(|&(at, micro)| {
            let offset = u128::try_from(at - first).unwrap_or(0);
            let x = f64::from(permille(offset, span)) * width / 1000.0;
            let y = y_for(micro, top, height);
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `y` of the cap line, on the same scale as the spend line.
pub fn cap_line_y(points: &[(i64, u64)], cap: u64, height: f64) -> f64 {
    y_for(cap, chart_top(points, cap), height)
}

/// `HH:MM:SS` in EAT for a unix time in seconds.
pub fn clock_eat(unix_seconds: i64) -> String {
    let day_seconds = (unix_seconds + EAT_OFFSET_SECONDS).rem_euclid(86_400);
    format!(
        "{:02}:{:02}:{:02}",
        day_seconds / 3600,
        day_seconds % 3600 / 60,
        day_seconds % 60
    )
}

#[component]
pub fn CostChart(
    #[prop(into)] points: Signal<Vec<(i64, u64)>>,
    #[prop(into)] cap: Signal<Cost>,
) -> impl IntoView {
    let enough = move || points.with(|list| list.len() >= 2);
    let line = move || polyline_points(&points.get(), WIDTH, HEIGHT, cap.get().micro_usd());
    let cap_y = move || cap_line_y(&points.get(), cap.get().micro_usd(), HEIGHT);
    view! {
        <Show when=enough fallback=|| view! { <div class="sub" style="margin-top:.5em">"The chart appears after two cost updates."</div> }>
            <svg viewBox=format!("0 0 {WIDTH} {HEIGHT}") style="width:100%;height:4.5em" role="img" aria-label="Cost over time">
                <line x1="0" x2=WIDTH.to_string() y1=cap_y y2=cap_y stroke="var(--danger)" stroke-dasharray="4 4" opacity=".7" />
                <polyline points=line fill="none" stroke="var(--accent)" stroke-width="2" />
            </svg>
            <details style="margin-top:.5em">
                <summary class="sub">"Table view"</summary>
                <table class="dl">
                    <tr><th>"Time (EAT)"</th><th>"Spend"</th></tr>
                    {move || points.get().into_iter().map(|(at, micro)| view! {
                        <tr>
                            <td class="mono">{clock_eat(at)}</td>
                            <td class="mono">{Cost::from_micro_usd(micro, false).to_string()}</td>
                        </tr>
                    }).collect_view()}
                </table>
            </details>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_point_is_at_the_left_and_the_last_at_the_right() {
        let points = [(100, 0), (200, 1_000_000)];
        let drawn = polyline_points(&points, 360.0, 60.0, 2_000_000);
        let coords: Vec<&str> = drawn.split(' ').collect();
        assert!(coords[0].starts_with("0.0,"), "{drawn}");
        assert!(coords[1].starts_with("360.0,"), "{drawn}");
    }

    #[test]
    fn spend_at_the_cap_sits_at_the_top_pad() {
        let points = [(0, 2_000_000)];
        let y = cap_line_y(&points, 2_000_000, 60.0);
        assert!((y - 2.0).abs() < 0.001, "{y}");
    }

    #[test]
    fn an_empty_series_draws_nothing() {
        assert_eq!(polyline_points(&[], 360.0, 60.0, 1), "");
    }

    #[test]
    fn the_scale_top_is_never_below_the_cap() {
        assert_eq!(chart_top(&[(0, 500)], 2_000), 2_000);
        assert_eq!(chart_top(&[(0, 5_000)], 2_000), 5_000);
        assert_eq!(chart_top(&[], 0), 1);
    }

    #[test]
    fn clock_is_shown_in_eat() {
        assert_eq!(clock_eat(0), "03:00:00");
        assert_eq!(clock_eat(-3 * 3600), "00:00:00");
    }
}
