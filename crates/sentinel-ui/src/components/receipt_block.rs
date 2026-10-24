//! `ReceiptBlock`: the commit that proves a VERIFIED task (`docs/ui-registry.md`, Board and Tasks).

use leptos::prelude::*;

#[component]
pub fn ReceiptBlock(receipt: Option<String>) -> impl IntoView {
    match receipt {
        Some(commit) => view! {
            <span class="mono" title="Commit that holds this task's work">{commit}</span>
        }
        .into_any(),
        None => view! { <span class="sub">"-"</span> }.into_any(),
    }
}
