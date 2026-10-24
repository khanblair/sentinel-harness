//! `PermissionMatrix`: one row per enabled agent, one switch per capability
//! (`docs/ui-registry.md`, Permissions).

use leptos::prelude::*;
use sentinel_core::{Capability, ToolId};

use crate::views::permissions::{CAPABILITY_COLUMNS, MatrixRow};

#[component]
pub fn PermissionMatrix(
    #[prop(into)] rows: Signal<Vec<MatrixRow>>,
    on_toggle: Callback<(ToolId, Capability)>,
) -> impl IntoView {
    view! {
        <div class="scrollx">
            <table class="dl">
                <tr>
                    <th>"Agent"</th>
                    {CAPABILITY_COLUMNS
                        .into_iter()
                        .map(|(_, full, short)| view! { <th title=full>{short}</th> })
                        .collect_view()}
                </tr>
                {move || {
                    rows.get()
                        .into_iter()
                        .map(|row| matrix_row(row, on_toggle))
                        .collect_view()
                }}
            </table>
        </div>
    }
}

fn matrix_row(row: MatrixRow, on_toggle: Callback<(ToolId, Capability)>) -> impl IntoView {
    let agent = row.agent.clone();
    let cells: Vec<_> = CAPABILITY_COLUMNS
        .into_iter()
        .zip(row.allowed)
        .map(|((capability, full, _), on)| {
            let agent = agent.clone();
            view! {
                <td>
                    <button class="tog" role="switch" aria-checked=on.to_string()
                        aria-label=format!("{full} for {agent}")
                        on:click=move |_| on_toggle.run((agent.clone(), capability))></button>
                </td>
            }
        })
        .collect();
    view! {
        <tr>
            <td><span class="row">{row.agent.to_string()}</span></td>
            {cells}
        </tr>
    }
}
