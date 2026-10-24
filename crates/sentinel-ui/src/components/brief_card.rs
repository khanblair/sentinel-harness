//! `BriefCard`: one brief in "Latest briefs" on the Routines page. The body keeps its line breaks.

use leptos::prelude::*;
use sentinel_core::routines::Brief;

use crate::routines::brief_time_text;

#[component]
pub fn BriefCard(brief: Brief) -> impl IntoView {
    let when = brief_time_text(&brief.created_at);
    let sent = brief.sent_to_telegram.then_some(" · sent to Telegram");
    view! {
        <div class="card" style="margin-bottom:.5em">
            <div class="row sp">
                <b>{brief.title}</b>
                <span class="sub">{when}{sent}</span>
            </div>
            <div class="sub" style="white-space:pre-line;margin-top:.3em">{brief.body}</div>
        </div>
    }
}
