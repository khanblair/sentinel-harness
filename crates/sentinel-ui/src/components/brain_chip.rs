//! The Brain chip: shows the Brain mode and opens the Brain menu.

use leptos::prelude::*;
use sentinel_core::BrainMode;

use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::state::{Align, PopoverKind, use_ui};

/// The lowercase name the Brain menu and the chip both show.
pub fn brain_label(mode: BrainMode) -> &'static str {
    match mode {
        BrainMode::Off => "off",
        BrainMode::ReadOnly => "read-only",
        BrainMode::Learn => "learn",
    }
}

#[component]
pub fn BrainChip() -> impl IntoView {
    let ui = use_ui();
    view! {
        <button id="brainChip" class="cbtn" aria-label="Brain mode"
            on:click=move |_| toggle_popover(ui, "brainChip", Align::Left, PopoverKind::Brain)>
            <Icon name="brain" size=15 />
            <span>{move || format!("Brain: {}", brain_label(ui.snapshot.with(|s| s.brain_mode)))}</span>
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brain_modes_use_the_documented_names() {
        assert_eq!(brain_label(BrainMode::Off), "off");
        assert_eq!(brain_label(BrainMode::ReadOnly), "read-only");
        assert_eq!(brain_label(BrainMode::Learn), "learn");
    }
}
