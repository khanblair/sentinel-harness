//! The Cost panel: spend against the cap, the 70 and 95 percent marks, per-agent cost, spend over
//! time, and the lower-cap form (`docs/ui-rules.md`, Cost Meter).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::LowerCap;
use sentinel_core::{Cost, Event, EventRecord};

use crate::api;
use crate::components::cost_chart::CostChart;
use crate::components::cost_meter::CostMeter;
use crate::components::top_bar::settle;
use crate::state::{ToastKind, Ui, use_ui};

/// The two thresholds on the meter, in percent of the cap.
pub fn percent_marks() -> [u8; 2] {
    [70, 95]
}

/// The bar color for a spend percent: danger from 95, warning from 70, accent before that.
pub fn bar_tone(percent: u8) -> &'static str {
    if percent >= 95 {
        "var(--danger)"
    } else if percent >= 70 {
        "var(--warn)"
    } else {
        ""
    }
}

/// `$1.20`, with ` est` after it when any part was estimated.
pub fn money_text(cost: Cost) -> String {
    if cost.is_estimated() {
        format!("{cost} est")
    } else {
        cost.to_string()
    }
}

/// A cap as text for the input, such as `2.00`. Parsing it back gives the same amount.
pub fn cap_input_text(cap: Cost) -> String {
    let micro = cap.micro_usd();
    format!("{}.{:02}", micro / 1_000_000, (micro % 1_000_000) / 10_000)
}

/// Accepts a dollar amount strictly below the current cap. Raising the cap is not offered.
pub fn lower_cap_valid(current: Cost, text: &str) -> Result<Cost, String> {
    let next =
        Cost::parse_usd(text).map_err(|_| "Enter a dollar amount, such as 1.50.".to_owned())?;
    if next.micro_usd() == 0 {
        return Err("The cap must be more than $0.00.".to_owned());
    }
    if next.micro_usd() >= current.micro_usd() {
        return Err(format!(
            "The cap can only go down. Enter less than {current}."
        ));
    }
    Ok(next)
}

/// Spend points for the chart: unix seconds and micro-dollars, oldest first.
pub fn spend_points(events: &[EventRecord]) -> Vec<(i64, u64)> {
    events
        .iter()
        .filter_map(|record| {
            let Event::CostUpdate { total, .. } = &record.event else {
                return None;
            };
            Some((record.at.unix_timestamp(), total.micro_usd()))
        })
        .collect()
}

/// The percent cost bar width of one agent, relative to the biggest spender.
pub fn share_percent(cost: Cost, largest: Cost) -> u8 {
    if largest.micro_usd() == 0 {
        return 0;
    }
    let percent = u128::from(cost.micro_usd()) * 100 / u128::from(largest.micro_usd());
    u8::try_from(percent).unwrap_or(100)
}

fn lower_cap(ui: Ui, draft: RwSignal<String>, error: RwSignal<Option<String>>) {
    let current = ui.snapshot.with_untracked(|s| s.cost.cap);
    let text = draft.get_untracked();
    match lower_cap_valid(current, &text) {
        Err(message) => error.set(Some(message)),
        Ok(next) => {
            error.set(None);
            let body = LowerCap {
                cap: cap_input_text(next),
            };
            spawn_local(async move {
                let result = api::post::<_, serde_json::Value>("/api/cap/lower", &body).await;
                if result.is_ok() {
                    ui.show_toast(format!("Cap lowered to {next}."), ToastKind::Ok);
                }
                settle(ui, result);
            });
        }
    }
}

#[component]
fn CapForm() -> impl IntoView {
    let ui = use_ui();
    let draft = RwSignal::new(String::new());
    let error: RwSignal<Option<String>> = RwSignal::new(None);
    let cap_text = Memo::new(move |_| ui.snapshot.with(|s| cap_input_text(s.cost.cap)));
    Effect::new(move |_| draft.set(cap_text.get()));
    view! {
        <div class="psec">
            <h5>"Lower the cap"</h5>
            <div class="row">
                <input class="field" type="number" step="0.01" min="0.01"
                    aria-label="New cap in dollars" style="max-width:8em"
                    prop:value=move || draft.get()
                    on:input=move |event| draft.set(event_target_value(&event)) />
                <button class="btn" on:click=move |_| lower_cap(ui, draft, error)>"Lower cap"</button>
            </div>
            <div class="sub" style="margin-top:.4em">"The cap can only go down. There is no way to raise it during a run."</div>
            {move || error.get().map(|text| view! { <div class="sub" style="color:var(--danger);margin-top:.4em">{text}</div> })}
        </div>
    }
}

#[component]
fn AgentCosts() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="psec" style="margin-top:1em">
            <h5>"By agent"</h5>
            {move || {
                let rows = ui.snapshot.with(|s| s.per_agent_cost.clone());
                if rows.is_empty() {
                    return view! { <div class="note">"No spend yet."</div> }.into_any();
                }
                let largest = rows.iter().map(|row| row.cost).max().unwrap_or(Cost::ZERO);
                rows.into_iter()
                    .map(|row| {
                        let pct = share_percent(row.cost, largest);
                        let estimated = row.cost.is_estimated();
                        view! {
                            <div style="margin:.5em 0">
                                <div class="row sp">
                                    <span class="row">{row.agent.to_string()}
                                        {estimated.then(|| view! { <span class="chip">"est"</span> })}
                                    </span>
                                    <span class="mono">{row.cost.to_string()}</span>
                                </div>
                                <div class="hbar" style="margin-top:.3em"><i style=format!("width:{pct}%")></i></div>
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </div>
    }
}

#[component]
pub fn CostPanel() -> impl IntoView {
    let ui = use_ui();
    let total = Memo::new(move |_| ui.snapshot.with(|s| s.cost.total));
    let cap = Memo::new(move |_| ui.snapshot.with(|s| s.cost.cap));
    let points = Memo::new(move |_| ui.snapshot.with(|s| spend_points(&s.events)));
    view! {
        <div class="big">
            {move || money_text(total.get())}
            <span style="font-size:.5em;color:var(--muted);font-weight:400">
                {move || format!(" of {} cap", cap.get())}
            </span>
        </div>
        <CostMeter total=total cap=cap />
        <CostChart points=points cap=cap />
        <AgentCosts />
        <CapForm />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usd(text: &str) -> Cost {
        Cost::parse_usd(text).unwrap()
    }

    #[test]
    fn marks_sit_at_70_and_95_percent() {
        assert_eq!(percent_marks(), [70, 95]);
    }

    #[test]
    fn bar_turns_warn_then_danger() {
        assert_eq!(bar_tone(10), "");
        assert_eq!(bar_tone(70), "var(--warn)");
        assert_eq!(bar_tone(95), "var(--danger)");
    }

    #[test]
    fn estimated_money_says_est() {
        assert_eq!(money_text(usd("1.20")), "$1.20");
        assert_eq!(
            money_text(Cost::from_micro_usd(1_200_000, true)),
            "$1.20 est"
        );
    }

    #[test]
    fn cap_text_round_trips() {
        assert_eq!(cap_input_text(usd("1.50")), "1.50");
        assert_eq!(cap_input_text(usd("2")), "2.00");
    }

    #[test]
    fn lower_cap_accepts_only_a_smaller_amount() {
        let current = usd("2.00");
        assert_eq!(lower_cap_valid(current, "1.50"), Ok(usd("1.50")));
        assert!(lower_cap_valid(current, "2.00").is_err());
        assert!(lower_cap_valid(current, "3").is_err());
        assert!(lower_cap_valid(current, "0").is_err());
        assert!(lower_cap_valid(current, "abc").is_err());
    }

    #[test]
    fn share_is_relative_to_the_largest_spender() {
        assert_eq!(share_percent(usd("1"), usd("4")), 25);
        assert_eq!(share_percent(usd("1"), Cost::ZERO), 0);
    }

    #[test]
    fn spend_points_come_from_cost_updates_only() {
        let events = vec![EventRecord {
            chat: None,
            id: 1,
            at: time::OffsetDateTime::UNIX_EPOCH,
            event: Event::CostUpdate {
                total: usd("0.50"),
                cap: usd("2"),
            },
        }];
        assert_eq!(spend_points(&events), vec![(0, 500_000)]);
    }
}
