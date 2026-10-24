//! `DryRunStages`: the six stages the dry run goes through, shown as a static preview. The daemon
//! does not run the dry run in this build (`POST /api/dry-run` answers 404), so no stage is
//! marked as done (`docs/product-scope.md`, F8 step 5).

use leptos::prelude::*;

use crate::components::status_badge::shape_markup;

/// The stage titles and what each one does, in order.
pub const DRY_STAGES: [(&str, &str); 6] = [
    (
        "Plan",
        "Sentinel reads the 3 sample tasks and decides which files each one needs.",
    ),
    (
        "Lock files",
        "Each task locks its files, so two agents can never edit the same file.",
    ),
    (
        "Agents work",
        "Two of your agents work at the same time, on different files.",
    ),
    ("Run tests", "The sample tests run on the finished work."),
    (
        "Review",
        "A different agent checks each change. Only then is a task marked Verified.",
    ),
    (
        "Report",
        "You get a report with a receipt for every verified task.",
    ),
];

#[component]
pub fn DryRunStages() -> impl IntoView {
    let ring = shape_markup("ring", "muted");
    view! {
        <div class="card">
            <b>"What happens"</b>
            <div style="margin-top:.3em">
                {DRY_STAGES
                    .into_iter()
                    .map(|(title, detail)| {
                        view! {
                            <div class="stg todo">
                                <span class="ico">
                                    <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=ring.clone()></svg>
                                </span>
                                <div>
                                    <b>{title}</b>
                                    <small>{detail}</small>
                                </div>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dry_run_has_six_stages_in_order() {
        let titles: Vec<&str> = DRY_STAGES.iter().map(|(title, _)| *title).collect();
        assert_eq!(
            titles,
            [
                "Plan",
                "Lock files",
                "Agents work",
                "Run tests",
                "Review",
                "Report"
            ]
        );
    }
}
