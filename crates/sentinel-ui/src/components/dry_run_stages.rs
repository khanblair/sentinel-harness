//! `DryRunStages`: the six stages of the dry run with the look each one has now. The looks come
//! from `dry_run::row_states`, which reads `GET /api/dry-run` (`docs/product-scope.md`, F8
//! step 5). A failed stage shows the last note the daemon sent under it.

use leptos::prelude::*;
use sentinel_core::dry_run::DryRunStage;

use crate::components::status_badge::shape_markup;
use crate::dry_run::{StageLook, stage_class, stage_look, stage_title};

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

/// The note under a failed stage. Other stages show no note.
fn failure_note(look: StageLook, note: Option<String>) -> Option<String> {
    match look {
        StageLook::Failed => note,
        StageLook::Waiting | StageLook::Running | StageLook::Done => None,
    }
}

#[component]
fn StageRow(
    stage: DryRunStage,
    look: StageLook,
    detail: &'static str,
    note: Option<String>,
) -> impl IntoView {
    let (shape, color) = stage_look(look);
    view! {
        <div class=format!("stg {}", stage_class(look))>
            <span class="ico">{stage_icon(shape, color)}</span>
            <div>
                <b>{stage_title(stage)}</b>
                <small>{detail}</small>
                {note.map(|text| view! { <div class="sub">{text}</div> })}
            </div>
        </div>
    }
}

#[component]
pub fn DryRunStages(looks: Vec<StageLook>, last_note: Option<String>) -> impl IntoView {
    view! {
        <div class="card">
            <b>"What happens"</b>
            <div style="margin-top:.3em">
                {DryRunStage::ALL
                    .into_iter()
                    .zip(DRY_STAGE_DETAILS)
                    .zip(looks)
                    .map(move |((stage, detail), look)| {
                        let note = failure_note(look, last_note.clone());
                        view! { <StageRow stage=stage look=look detail=detail note=note /> }
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
    fn only_a_failed_stage_shows_the_note() {
        let note = Some("T-04 went back for another try".to_string());
        assert_eq!(failure_note(StageLook::Failed, note.clone()), note);
        assert_eq!(failure_note(StageLook::Done, note.clone()), None);
        assert_eq!(failure_note(StageLook::Running, note.clone()), None);
        assert_eq!(failure_note(StageLook::Waiting, note), None);
    }

    #[test]
    fn a_failed_stage_with_no_notes_shows_no_note() {
        assert_eq!(failure_note(StageLook::Failed, None), None);
    }
}
