//! `GuideModal`: the thirteen-step guide. `ModalHost` draws the dialog frame. This is its body.

use leptos::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use crate::components::icon::Icon;
use crate::guide::{GUIDE_STEPS, GuideAction, GuideStep, next_index, previous_index};
use crate::state::{Modal, Ui, use_ui};

const COMPOSER_ID: &str = "cin";

#[component]
pub fn GuideModal() -> impl IntoView {
    let ui = use_ui();
    let index = RwSignal::new(0_usize);
    view! {
        <div class="modal-hd">
            <Icon name="book" size=18 />
            <h3>"How to use Sentinel"</h3>
            <button class="ibtn" aria-label="Close guide" on:click=move |_| ui.modal.set(None)>
                <Icon name="x" size=17 />
            </button>
        </div>
        <div class="modal-body">
            <div class="guide">
                <nav class="g-list" aria-label="Guide steps">
                    {(0..GUIDE_STEPS.len()).map(|step| step_button(index, step)).collect_view()}
                </nav>
                <div class="g-body">{move || guide_body(ui, index)}</div>
            </div>
        </div>
    }
}

fn step_button(index: RwSignal<usize>, step: usize) -> impl IntoView {
    let title = GUIDE_STEPS.get(step).map_or("", |item| item.title);
    view! {
        <button aria-current=move || (index.get() == step).to_string() on:click=move |_| index.set(step)>
            <span class="n">{step + 1}</span>
            <span>{title}</span>
        </button>
    }
}

fn guide_body(ui: Ui, index: RwSignal<usize>) -> AnyView {
    let current = index.get();
    match GUIDE_STEPS.get(current) {
        Some(step) => step_view(ui, index, current, *step).into_any(),
        None => ().into_any(),
    }
}

fn step_view(ui: Ui, index: RwSignal<usize>, current: usize, step: GuideStep) -> impl IntoView {
    let total = GUIDE_STEPS.len();
    let last = current + 1 == total;
    view! {
        <div class="flow">{flow_items(current)}</div>
        <h2>{format!("{}. {}", current + 1, step.title)}</h2>
        <p inner_html=step.body></p>
        {(!step.tries.is_empty()).then(|| try_row(ui, step))}
        <div class="row" style="margin-top:1.4em">
            <button class="btn" disabled=current == 0
                on:click=move |_| index.set(previous_index(current))>"Previous"</button>
            <button class="btn" disabled=last
                on:click=move |_| index.set(next_index(current, total))>"Next step"</button>
        </div>
        {last.then(rules_of_thumb)}
    }
}

/// The step flow strip. The current step is `on`, as in the prototype.
fn flow_items(current: usize) -> impl IntoView {
    let total = GUIDE_STEPS.len();
    GUIDE_STEPS
        .iter()
        .enumerate()
        .map(|(step, item)| {
            let class = if step == current { "on" } else { "" };
            view! {
                <span class=class>{item.title}</span>
                {(step + 1 < total).then(|| view! { <i>"›"</i> })}
            }
        })
        .collect_view()
}

fn try_row(ui: Ui, step: GuideStep) -> impl IntoView {
    let buttons = step
        .tries
        .iter()
        .map(|button| {
            let action = button.action;
            view! {
                <button class="btn pri" on:click=move |_| run_guide_action(ui, action)>
                    {button.label}
                </button>
            }
        })
        .collect_view();
    view! { <div class="row wrap">{buttons}</div> }
}

/// Closes the guide, then does the action. Settings replaces the guide, so the order matters.
fn run_guide_action(ui: Ui, action: GuideAction) {
    ui.modal.set(None);
    match action {
        GuideAction::Settings(page) => ui.modal.set(Some(Modal::Settings {
            page: page.to_owned(),
        })),
        GuideAction::Panel(panel) => ui.open_panel(panel),
        GuideAction::FocusComposer => focus_composer(),
    }
}

fn focus_composer() {
    let composer = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(COMPOSER_ID))
        .and_then(|element| element.dyn_into::<HtmlElement>().ok());
    if let Some(composer) = composer {
        let _ = composer.focus();
    }
}

fn rules_of_thumb() -> impl IntoView {
    view! {
        <div class="card" style="margin-top:1.2em">
            <b>"Rules of thumb"</b>
            <ul style="margin:.4em 0 0 1.1em;padding:0;line-height:1.7">
                <li>"Start with plan only until you trust the plan."</li>
                <li>"Keep the cap low. You can only lower it."</li>
                <li>"Never edit tests to make a task pass."</li>
                <li>"If a task says Blocked, read the reason before retrying."</li>
            </ul>
        </div>
    }
}
