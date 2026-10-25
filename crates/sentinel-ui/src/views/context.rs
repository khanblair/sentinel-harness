//! The Context panel: what fills the agent's context window for the open chat. Counts are
//! estimates from the daemon, so the panel says so and refreshes on its own.

use std::time::Duration;

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::ChatId;
use sentinel_core::context::{ContextEntry, ContextItem, ContextUsage};

use crate::api;
use crate::state::{Ui, use_ui};

/// How often the panel asks the daemon again while it is open.
const REFRESH_EVERY: Duration = Duration::from_secs(10);

/// The request path for the open chat. Without a chat the daemon picks one itself.
pub fn context_path(chat: Option<&ChatId>) -> String {
    match chat {
        Some(id) => format!("/api/context?chat={}", id.as_str()),
        None => "/api/context".to_owned(),
    }
}

/// A token count for people: `999`, `1k`, `65.2k`, `128k`, `1M`.
pub fn format_tokens(tokens: u32) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }
    if tokens < 1_000_000 {
        return scaled(tokens, 1_000, "k");
    }
    scaled(tokens, 1_000_000, "M")
}

/// One decimal at most, and no decimal when it is zero.
fn scaled(tokens: u32, unit: u32, suffix: &str) -> String {
    let tenths = u64::from(tokens) * 10 / u64::from(unit);
    let whole = tenths / 10;
    match tenths % 10 {
        0 => format!("{whole}{suffix}"),
        frac => format!("{whole}.{frac}{suffix}"),
    }
}

/// The share of the window, in percent. A window of zero gives zero, never a division error.
pub fn percent_of(tokens: u32, window: u32) -> f64 {
    if window == 0 {
        return 0.0;
    }
    f64::from(tokens) * 100.0 / f64::from(window)
}

/// The color of an item's square and bar segment. Free space has no fill.
fn item_color(kind: &str) -> &'static str {
    match kind {
        "messages" => "var(--accent)",
        "instructions" => "var(--info)",
        "brain" => "var(--ok)",
        "skills" => "var(--lock)",
        "attachments" => "var(--warn)",
        _ => "var(--muted)",
    }
}

/// The inline style of an item's square in a row.
fn swatch_style(kind: &str) -> String {
    let shape = "display:inline-block;flex:none;width:.7em;height:.7em;border-radius:2px";
    match kind {
        "free" => format!("{shape};border:1px solid var(--muted)"),
        _ => format!("{shape};background:{}", item_color(kind)),
    }
}

/// The inline style of one bar segment. A segment with tokens never shrinks below 2 px.
fn segment_style(item: &ContextItem, window: u32) -> String {
    let width = percent_of(item.tokens, window);
    let min_width = if item.tokens > 0 { "2px" } else { "0" };
    format!(
        "width:{width:.2}%;min-width:{min_width};background:{}",
        item_color(&item.kind)
    )
}

/// The bar shows every item except free space.
fn bar_items(usage: &ContextUsage) -> impl Iterator<Item = &ContextItem> {
    usage.items.iter().filter(|item| item.kind != "free")
}

fn load(ui: Ui, usage: RwSignal<Option<ContextUsage>>, failure: RwSignal<Option<String>>) {
    let asked = ui.open_chat.get_untracked();
    spawn_local(async move {
        let result = api::get::<ContextUsage>(&context_path(asked.as_ref())).await;
        // A chat switched while the request was out must not show the old chat's numbers.
        if ui.open_chat.get_untracked() != asked {
            return;
        }
        match result {
            Ok(next) => {
                usage.set(Some(next));
                failure.set(None);
            }
            Err(error) => failure.set(Some(error.to_string())),
        }
    });
}

fn keep_refreshing(
    ui: Ui,
    usage: RwSignal<Option<ContextUsage>>,
    failure: RwSignal<Option<String>>,
) {
    let tick = set_interval_with_handle(move || load(ui, usage, failure), REFRESH_EVERY);
    if let Ok(tick) = tick {
        on_cleanup(move || tick.clear());
    }
}

#[component]
pub fn ContextPanel() -> impl IntoView {
    let ui = use_ui();
    let usage: RwSignal<Option<ContextUsage>> = RwSignal::new(None);
    let failure: RwSignal<Option<String>> = RwSignal::new(None);
    Effect::new(move |_| {
        ui.open_chat.track();
        load(ui, usage, failure);
    });
    keep_refreshing(ui, usage, failure);
    view! {
        <ContextSummary usage=usage />
        <ContextBar usage=usage />
        <ContextRows usage=usage />
        <ContextLists usage=usage />
        {move || failure.get().map(|text| view! { <div class="note" style="color:var(--danger)">{text}</div> })}
    }
}

#[component]
fn ContextSummary(usage: RwSignal<Option<ContextUsage>>) -> impl IntoView {
    view! {
        <div class="card">
            <div class="row sp">
                <div class="big">
                    {move || usage.get().map_or("Loading".to_owned(), |u| {
                        format!("{:.0}% used", percent_of(u.used_tokens, u.window_tokens))
                    })}
                </div>
                <span class="mono sub">
                    {move || usage.get().map_or(String::new(), |u| {
                        format!("{} / {}", format_tokens(u.used_tokens), format_tokens(u.window_tokens))
                    })}
                </span>
            </div>
            <div class="sub">
                {move || usage.get().map_or(String::new(), |u| {
                    format!("{}, estimate. One token is about four characters.", u.agent)
                })}
            </div>
        </div>
    }
}

#[component]
fn ContextBar(usage: RwSignal<Option<ContextUsage>>) -> impl IntoView {
    view! {
        <div class="hbar" style="display:flex;margin:.8em 0 .4em">
            {move || usage.get().map(|u| {
                bar_items(&u)
                    .map(|item| view! { <i style=segment_style(item, u.window_tokens)></i> })
                    .collect_view()
            })}
        </div>
        {move || usage.get().filter(|u| u.used_tokens == 0).map(|_| {
            view! { <div class="note">"Nothing is using the window yet."</div> }
        })}
    }
}

#[component]
fn ContextRows(usage: RwSignal<Option<ContextUsage>>) -> impl IntoView {
    view! {
        <div style="margin-top:.4em">
            {move || usage.get().map(|u| {
                u.items
                    .into_iter()
                    .map(|item| context_row(item, u.window_tokens))
                    .collect_view()
            })}
        </div>
    }
}

/// One row: a square, the label, the tokens, and the percent of the window.
fn context_row(item: ContextItem, window: u32) -> impl IntoView {
    let swatch = swatch_style(&item.kind);
    let percent = format!("{:.1}%", percent_of(item.tokens, window));
    let label = item.label;
    let tokens = format_tokens(item.tokens);
    view! {
        <div class="row sp" style="margin:.35em 0">
            <span class="row">
                <i style=swatch></i>
                <span>{label}</span>
            </span>
            <span class="row">
                <span class="mono">{tokens}</span>
                <span class="mono sub" style="min-width:4.2em;text-align:right">{percent}</span>
            </span>
        </div>
    }
}

#[component]
fn ContextLists(usage: RwSignal<Option<ContextUsage>>) -> impl IntoView {
    view! {
        {move || usage.get().map(|u| {
            view! {
                {entry_list("Brain notes", u.notes)}
                {entry_list("Skills", u.skills)}
                {entry_list("Attachments", u.attachments)}
            }
        })}
    }
}

/// A collapsed list of entries. It is left out when it has none.
fn entry_list(title: &'static str, entries: Vec<ContextEntry>) -> Option<impl IntoView> {
    (!entries.is_empty()).then(move || {
        let rows = entries
            .iter()
            .map(|entry| {
                let name = entry.name.clone();
                let tokens = format_tokens(entry.tokens);
                view! {
                    <div class="row sp" style="margin:.25em 0">
                        <span class="sub">{name}</span>
                        <span class="mono sub">{tokens}</span>
                    </div>
                }
            })
            .collect_view();
        view! {
            <details style="margin-top:.6em">
                <summary>{format!("{title} ({})", entries.len())}</summary>
                <div style="margin-top:.3em">{rows}</div>
            </details>
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_counts_stay_whole() {
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(0), "0");
    }

    #[test]
    fn thousands_show_one_decimal_only_when_it_is_not_zero() {
        assert_eq!(format_tokens(1_000), "1k");
        assert_eq!(format_tokens(65_200), "65.2k");
        assert_eq!(format_tokens(125_900), "125.9k");
        assert_eq!(format_tokens(128_000), "128k");
    }

    #[test]
    fn millions_show_as_m() {
        assert_eq!(format_tokens(1_000_000), "1M");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn a_zero_window_gives_zero_percent() {
        assert!(percent_of(500, 0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_percent_is_the_share_of_the_window() {
        assert!((percent_of(25_000, 200_000) - 12.5).abs() < 1e-9);
    }

    #[test]
    fn the_path_names_the_chat_only_when_there_is_one() {
        assert_eq!(context_path(None), "/api/context");
        assert_eq!(
            context_path(Some(&ChatId::new("chat-1"))),
            "/api/context?chat=chat-1"
        );
    }

    #[test]
    fn a_segment_with_tokens_is_at_least_two_pixels_wide() {
        let item = ContextItem {
            kind: "brain".to_owned(),
            label: "Brain notes".to_owned(),
            tokens: 1,
        };
        assert!(segment_style(&item, 1_000_000).contains("min-width:2px"));
    }

    #[test]
    fn free_space_has_an_outline_and_no_fill() {
        assert!(swatch_style("free").contains("border:1px solid"));
        assert!(!swatch_style("free").contains("background:"));
    }
}
