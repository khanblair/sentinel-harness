//! The permission chip: shows the mode and opens the mode menu. Bypass uses the warn color.

use leptos::prelude::*;
use sentinel_core::PermissionMode;

use crate::components::icon::Icon;
use crate::components::popover::toggle_popover;
use crate::state::{Align, PopoverKind, use_ui};

pub fn chip_class(mode: PermissionMode) -> &'static str {
    if mode == PermissionMode::Bypass {
        "cbtn warn"
    } else {
        "cbtn"
    }
}

/// The same icon the mode menu shows for each mode.
pub fn mode_icon(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::PlanOnly => "eye",
        PermissionMode::AskEachWave => "shield",
        PermissionMode::AutoInsideReserved => "lock",
        PermissionMode::Bypass => "alert",
    }
}

#[component]
pub fn PermissionChip() -> impl IntoView {
    let ui = use_ui();
    let mode = move || ui.snapshot.with(|s| s.permission_mode);
    view! {
        <button id="permChip" class=move || chip_class(mode()) aria-label="Permission mode"
            on:click=move |_| toggle_popover(ui, "permChip", Align::Left, PopoverKind::Permission)>
            {move || view! { <Icon name=mode_icon(mode()) size=15 /> }}
            <span>{move || mode().label()}</span>
            <Icon name="down" size=12 />
        </button>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bypass_uses_the_warn_color() {
        assert_eq!(chip_class(PermissionMode::Bypass), "cbtn warn");
        assert_eq!(chip_class(PermissionMode::PlanOnly), "cbtn");
        assert_eq!(chip_class(PermissionMode::AutoInsideReserved), "cbtn");
    }

    #[test]
    fn each_mode_has_its_menu_icon() {
        assert_eq!(mode_icon(PermissionMode::PlanOnly), "eye");
        assert_eq!(mode_icon(PermissionMode::AskEachWave), "shield");
        assert_eq!(mode_icon(PermissionMode::AutoInsideReserved), "lock");
        assert_eq!(mode_icon(PermissionMode::Bypass), "alert");
    }
}
