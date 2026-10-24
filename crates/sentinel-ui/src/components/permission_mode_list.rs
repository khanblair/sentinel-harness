//! `PermissionModeList`: the four permission mode cards (`docs/ui-registry.md`, Permissions).

use leptos::prelude::*;
use sentinel_core::PermissionMode;

use crate::components::icon::Icon;
use crate::views::permissions::{MODES, mode_copy};

#[component]
pub fn PermissionModeList(
    #[prop(into)] selected: Signal<PermissionMode>,
    on_pick: Callback<PermissionMode>,
) -> impl IntoView {
    view! {
        <div class="radios" style="margin-bottom:1em">
            {MODES
                .into_iter()
                .map(|mode| {
                    let copy = mode_copy(mode);
                    view! {
                        <button aria-pressed=move || (selected.get() == mode).to_string()
                            on:click=move |_| on_pick.run(mode)>
                            <span><Icon name=copy.icon size=18 /></span>
                            <div>
                                <b>{mode.label()}</b>
                                <div class="d">{copy.description}</div>
                            </div>
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
