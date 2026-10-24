//! The Proposals tab: the open proposals from care runs, or the empty card with Run care now
//! (computer only).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::ProposalView;

use crate::api;
use crate::components::icon::Icon;
use crate::components::proposal_list::ProposalList;
use crate::views::brain::{BrainCtx, report_failure, run_care};

#[component]
pub fn ProposalsTab(ctx: BrainCtx) -> impl IntoView {
    let proposals = RwSignal::new(None::<Vec<ProposalView>>);
    Effect::new(move |_| {
        ctx.revision.track();
        load_proposals(ctx, proposals);
    });
    move || match proposals.get() {
        None => ().into_any(),
        Some(list) if list.is_empty() => empty_card(ctx).into_any(),
        Some(list) => view! { <ProposalList proposals=list ctx=ctx /> }.into_any(),
    }
}

fn load_proposals(ctx: BrainCtx, proposals: RwSignal<Option<Vec<ProposalView>>>) {
    spawn_local(async move {
        match api::get::<Vec<ProposalView>>("/api/brain/proposals").await {
            Ok(list) => {
                ctx.open_proposals.set(list.len());
                proposals.set(Some(list));
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
}

fn empty_card(ctx: BrainCtx) -> impl IntoView {
    view! {
        <div class="card">
            <b>"No proposals"</b>
            <div class="sub" style="margin:.3em 0 .6em">
                "Brain care proposes new notes, updates, merges and retirements. Nothing changes until you accept it."
            </div>
            {move || (!ctx.ui.on_phone()).then(|| view! {
                <button class="btn pri" on:click=move |_| run_care(ctx.ui, ctx.missing)>
                    <Icon name="play" size=13 />" Run Brain care now"
                </button>
            })}
        </div>
    }
}
