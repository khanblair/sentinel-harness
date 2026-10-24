//! The stream: the run's items in order, built from the snapshot. The view follows the newest
//! item while the reader is near the bottom. The focus bar lives in the shell (`app.rs`).

use leptos::html;
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{Element, Event as DomEvent, HtmlElement};

use crate::components::agent_step_card::AgentStepCard;
use crate::components::approval_card::ApprovalCard;
use crate::components::barrier_card::BarrierCard;
use crate::components::blocked_write_card::{BlockedWriteCard, blocked_copy};
use crate::components::budget_banner::BudgetBanner;
use crate::components::decision_card::DecisionCard;
use crate::components::empty_run_state::EmptyChatState;
use crate::components::file_change_card::FileChangeCard;
use crate::components::message_cards::{NoteLine, SystemMessage, UserMessage};
use crate::components::plan_card::PlanCard;
use crate::components::report_card::ReportCard;
use crate::components::verdict_card::VerdictCard;
use crate::components::wave_card::WaveCard;
use crate::state::{Ui, use_ui};
use crate::views::stream_model::{Item, build_items, token_label};

/// Within this many pixels of the bottom, new items pull the view down.
const NEAR_BOTTOM_PX: i32 = 160;

/// Whether the reader is close enough to the bottom to follow new items.
pub fn near_bottom(scroll_height: i32, scroll_top: i32, client_height: i32) -> bool {
    scroll_height - scroll_top - client_height < NEAR_BOTTOM_PX
}

#[component]
pub fn StreamView() -> impl IntoView {
    let ui = use_ui();
    let items = Memo::new(move |_| {
        ui.snapshot
            .with(|s| build_items(s, ui.focus.get().as_ref()))
    });
    let anchor = NodeRef::<html::Div>::new();
    let pinned = RwSignal::new(true);
    Effect::new(move |_| {
        if let Some(element) = anchor.get() {
            follow_scroll(&element, pinned);
        }
    });
    Effect::new(move |_| {
        items.track();
        let following = pinned.get_untracked();
        if let Some(element) = anchor.get_untracked().filter(|_| following) {
            // The new cards are in the page after this effect, so scroll on the next frame.
            request_animation_frame(move || scroll_to_end(&element));
        }
    });
    view! {
        {move || empty_or_note(ui, items)}
        <For
            each=move || items.get()
            key=|(id, item)| (*id, item.clone())
            children=|(id, item)| view! { <div class="ev" data-id=id.to_string()>{item_view(item)}</div> }
        />
        <div node_ref=anchor></div>
    }
}

/// With no items, a fresh run gets its first actions. A focused agent with no items gets a note.
fn empty_or_note(ui: Ui, items: Memo<Vec<(i64, Item)>>) -> AnyView {
    if !items.with(Vec::is_empty) {
        return ().into_any();
    }
    if ui.focus.get().is_some() {
        view! { <NoteLine text="Nothing from this agent yet." icon="eye" /> }.into_any()
    } else {
        view! { <EmptyChatState /> }.into_any()
    }
}

/// One view per item. Each stage takes the items it knows and passes the rest on.
fn item_view(item: Item) -> AnyView {
    if let Item::User { to, text, .. } = item {
        return view! { <UserMessage to=to text=text /> }.into_any();
    }
    if let Item::System { text } = item {
        return view! { <SystemMessage text=text /> }.into_any();
    }
    if let Item::Note { text, icon } = item {
        return view! { <NoteLine text=text icon=icon /> }.into_any();
    }
    if let Item::Plan {
        tasks,
        contradictions,
        already_done,
    } = item
    {
        return view! {
            <PlanCard tasks=tasks contradictions=contradictions already_done=already_done />
        }
        .into_any();
    }
    if let Item::Wave { wave, tasks } = item {
        return view! { <WaveCard wave=wave tasks=tasks /> }.into_any();
    }
    work_view(item)
}

fn work_view(item: Item) -> AnyView {
    if let Item::Agent { task, agent, state } = item {
        return view! { <AgentStepCard task=task agent=agent state=state /> }.into_any();
    }
    if let Item::Checkpoint { commit, paths, .. } = item {
        return paths
            .into_iter()
            .map(|path| view! { <FileChangeCard path=path commit=commit.clone() /> })
            .collect_view()
            .into_any();
    }
    if let Item::Approval { id } = item {
        return view! { <ApprovalCard id=id /> }.into_any();
    }
    if let Item::Decision { id } = item {
        return view! { <DecisionCard id=id /> }.into_any();
    }
    if let Item::BlockedWrite { path, restored } = item {
        let text = blocked_copy(&path, restored);
        return view! { <BlockedWriteCard text=text /> }.into_any();
    }
    status_view(item)
}

fn status_view(item: Item) -> AnyView {
    if let Item::Barrier { wave, state } = item {
        return view! { <BarrierCard wave=wave state=state /> }.into_any();
    }
    if let Item::Verdict {
        task,
        status,
        reason,
    } = item
    {
        return view! { <VerdictCard task=task status=status reason=reason /> }.into_any();
    }
    if let Item::Budget { percent } = item {
        return view! { <BudgetBanner percent=percent /> }.into_any();
    }
    if let Item::Ceiling { task } = item {
        let text = format!("{task} stopped at its cost ceiling.");
        return view! { <NoteLine text=text icon="dollar" /> }.into_any();
    }
    if let Item::Compacted { before, after, .. } = item {
        let text = format!(
            "Context compacted: {} to {} tokens.",
            token_label(before),
            token_label(after)
        );
        return view! { <NoteLine text=text icon="spark" /> }.into_any();
    }
    if let Item::Report { verified, total } = item {
        return view! { <ReportCard verified=verified total=total /> }.into_any();
    }
    ().into_any()
}

fn scroll_container(element: &Element) -> Option<Element> {
    element.closest(".stream").ok().flatten()
}

/// Keeps `pinned` in step with the reader's position in the scrolling `.stream` container.
fn follow_scroll(anchor: &Element, pinned: RwSignal<bool>) {
    let Some(container) = scroll_container(anchor) else {
        return;
    };
    let watched = container.clone();
    let on_scroll = Closure::<dyn FnMut(DomEvent)>::new(move |_: DomEvent| {
        pinned.set(near_bottom(
            watched.scroll_height(),
            watched.scroll_top(),
            watched.client_height(),
        ));
    });
    let _ =
        container.add_event_listener_with_callback("scroll", on_scroll.as_ref().unchecked_ref());
    on_scroll.forget();
}

fn scroll_to_end(anchor: &HtmlElement) {
    if let Some(container) = scroll_container(anchor) {
        container.set_scroll_top(container.scroll_height());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_within_160_pixels_of_the_end_is_followed() {
        assert!(near_bottom(1000, 800, 100));
        assert!(!near_bottom(1000, 600, 100));
    }
}
