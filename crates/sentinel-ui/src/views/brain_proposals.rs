//! The Proposals tab: the open proposals from care runs, the Run care now button (computer only),
//! and the empty text when nothing is open. Accept and Dismiss reload the list through `bump`.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::ProposalView;

use crate::api;
use crate::brain::{
    LOADING_PROPOSALS, LoadState, NOTHING_TO_REVIEW_TEXT, RUN_CARE_LABEL, care_offered,
};
use crate::components::icon::Icon;
use crate::components::proposal_list::ProposalList;
use crate::views::brain::{BrainCtx, bump, fail_load, load_notice, run_care};

#[component]
pub fn ProposalsTab(ctx: BrainCtx) -> impl IntoView {
    let proposals = RwSignal::new(LoadState::Loading);
    Effect::new(move |_| {
        ctx.revision.track();
        load_proposals(ctx, proposals);
    });
    view! {
        {move || care_offered(ctx.ui.on_phone()).then(|| view! {
            <div class="row" style="margin-bottom:.6em">
                <button class="btn sm" on:click=move |_| run_care(ctx.ui, ctx.missing, move || bump(ctx))>
                    <Icon name="play" size=13 />" "{RUN_CARE_LABEL}
                </button>
            </div>
        })}
        {move || match proposals.get() {
            LoadState::Ready(list) if list.is_empty() => empty_card().into_any(),
            LoadState::Ready(list) => view! { <ProposalList proposals=list ctx=ctx /> }.into_any(),
            state @ (LoadState::Loading | LoadState::Failed(_)) => {
                load_notice(&state, LOADING_PROPOSALS)
            }
        }}
    }
}

fn load_proposals(ctx: BrainCtx, proposals: RwSignal<LoadState<Vec<ProposalView>>>) {
    spawn_local(async move {
        match api::get::<Vec<ProposalView>>("/api/brain/proposals").await {
            Ok(list) => {
                ctx.open_proposals.set(list.len());
                proposals.set(LoadState::Ready(list));
            }
            Err(failure) => fail_load(ctx, proposals, &failure),
        }
    });
}

fn empty_card() -> impl IntoView {
    view! {
        <div class="card">
            <b>"No proposals"</b>
            <div class="sub" style="margin-top:.3em">{NOTHING_TO_REVIEW_TEXT}</div>
        </div>
    }
}
