//! `DryRunStages`: the six stages of the dry run with the state each one has now. A stage with no
//! row has not started yet. The rows come from `GET /api/dry-run` (`docs/product-scope.md`, F8
//! step 5).

use leptos::prelude::*;
use sentinel_core::dry_run::{DryRunStage, DryRunStageRow, StageState};

use crate::components::status_badge::shape_markup;
use crate::dry_run::{stage_class, stage_look, stage_title, state_of};

/// What each stage does, in run order. The titles come from `dry_run::stage_title`.
pub const DRY_STAGE_DETAILS: [&str; 6] = [
    "Sentinel reads the 3 sample tasks and decides which files each one needs.",
    "Each task locks its files, so two agents can never edit the same file.",
    "Two of your agents work at the same time, on different files.",
    "The sample tests run on the finished work.",
    "A different agent checks each change. Only then is a task marked Verified.",
    "You get a report with a receipt for every verified task.",
];

/// One stage's icon. A running stage spins. Every other stage is a shape in a color.
fn stage_icon(shape: &str, color: &str) -> AnyView {
    if shape == "spin" {
        view! { <span class="spin"></span> }.into_any()
    } else {
        view! {
            <svg
                class="shp"
                viewBox="0 0 12 12"
                aria-hidden="true"
                inner_html=shape_markup(shape, color)
            ></svg>
        }
        .into_any()
    }
}

#[component]
fn StageRow(stage: DryRunStage, state: Option<StageState>, detail: &'static str) -> impl IntoView {
    let (shape, color) = stage_look(state);
    view! {
        <div class=format!("stg {}", stage_class(state))>
            <span class="ico">{stage_icon(shape, color)}</span>
            <div>
                <b>{stage_title(stage)}</b>
                <small>{detail}</small>
            </div>
        </div>
    }
}

#[component]
pub fn DryRunStages(rows: Vec<DryRunStageRow>) -> impl IntoView {
    view! {
        <div class="card">
            <b>"What happens"</b>
            <div style="margin-top:.3em">
                {DryRunStage::ALL
                    .into_iter()
                    .zip(DRY_STAGE_DETAILS)
                    .map(move |(stage, detail)| {
                        view! { <StageRow stage=stage state=state_of(&rows, stage) detail=detail /> }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}
