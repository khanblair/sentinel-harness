//! The command list under the composer. The arrow keys and Tab are handled by the composer, which
//! owns the text. This component only draws the list and reports a click.

use leptos::prelude::*;

use crate::slash::SlashEntry;

#[component]
pub fn SlashMenu(
    commands: Memo<Vec<SlashEntry>>,
    selected: RwSignal<usize>,
    #[prop(into)] on_pick: Callback<&'static str>,
) -> impl IntoView {
    view! {
        <div class="slash" id="slash" hidden=move || commands.with(Vec::is_empty)>
            <div class="pop" style="position:static;max-width:none">
                {move || {
                    commands
                        .get()
                        .into_iter()
                        .enumerate()
                        .map(|(index, (name, description))| view! {
                            <button class="mi" class:on=move || selected.get() == index
                                on:click=move |_| on_pick.run(name)>
                                <span class="mono">{name}</span>
                                <span class="d" style="margin:0 0 0 .6em">{description}</span>
                            </button>
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}
