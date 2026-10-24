//! `ProposalList`: one card per proposal from a care run, with Accept and Dismiss. Nothing changes
//! until Accept. A phone may accept or dismiss too (`docs/mobile.md`).

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::brain_api::{AnswerProposal, ProposalView};

use crate::api;
use crate::brain::{answer_done_text, proposal_kind_label};
use crate::state::ToastKind;
use crate::views::brain::{BrainCtx, bump, report_failure};

#[component]
pub fn ProposalList(proposals: Vec<ProposalView>, ctx: BrainCtx) -> impl IntoView {
    proposals
        .into_iter()
        .map(|proposal| proposal_card(proposal, ctx))
        .collect_view()
}

fn proposal_card(proposal: ProposalView, ctx: BrainCtx) -> impl IntoView {
    let ProposalView {
        id,
        kind,
        title,
        why,
        ..
    } = proposal;
    let accept_id = id.clone();
    let dismiss_id = id.clone();
    view! {
        <div class="tcard">
            <div class="row sp">
                <span class="chip a">{proposal_kind_label(kind)}</span>
                <span class="mono sub">{id}</span>
            </div>
            <div class="ttl">{title}</div>
            <div class="sub">{why}</div>
            <div class="row" style="margin-top:.5em;gap:.4em">
                <button class="btn sm pri" on:click=move |_| answer(ctx, &accept_id, true)>
                    "Accept"
                </button>
                <button class="btn sm" on:click=move |_| answer(ctx, &dismiss_id, false)>
                    "Dismiss"
                </button>
            </div>
        </div>
    }
}

fn answer(ctx: BrainCtx, id: &str, accept: bool) {
    let path = format!("/api/brain/proposals/{id}");
    spawn_local(async move {
        match api::post::<_, serde_json::Value>(&path, &AnswerProposal { accept }).await {
            Ok(_) => {
                ctx.ui.show_toast(answer_done_text(accept), ToastKind::Ok);
                bump(ctx);
            }
            Err(failure) => report_failure(ctx.ui, ctx.missing, &failure),
        }
    });
}
