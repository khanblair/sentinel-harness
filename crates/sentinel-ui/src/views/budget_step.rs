//! The budget step of the setup wizard: the cap, the permission mode, the Brain mode, and which
//! agent handles each tier (`docs/product-scope.md`, F8 step 3). Its shapes match the prototype's
//! budget page. It is a local copy until `components/budget_form.rs` (U-D) lands.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::{PermissionsUpdate, RoutingUpdate, SetBrainMode};
use sentinel_core::{BrainMode, PermissionMode, Routing, Tier, ToolId, ToolInfo};
use serde::Serialize;

use crate::api;
use crate::components::icon::Icon;
use crate::state::{Modal, ToastKind, Ui, refresh, use_ui};

/// The body of `PUT /api/config/cap`. The cap is dollars as text, such as `1.50`.
#[derive(Debug, Clone, Serialize)]
pub struct CapUpdate {
    pub cap: String,
}

/// The routing tiers in the order the step shows them, with their labels.
const TIERS: [(Tier, &str, &str); 3] = [
    (Tier::Cheap, "Easy tasks", "Tried first. Keeps cost low."),
    (Tier::Mid, "Normal tasks", "Used when an easy task fails."),
    (Tier::Smart, "Hard tasks", "Hard work, and the last resort."),
];

const PERMISSION_MODES: [PermissionMode; 4] = [
    PermissionMode::PlanOnly,
    PermissionMode::AskEachWave,
    PermissionMode::AutoInsideReserved,
    PermissionMode::Bypass,
];

const BRAIN_MODES: [BrainMode; 3] = [BrainMode::Off, BrainMode::ReadOnly, BrainMode::Learn];

/// One option card: a label, a line of description, and an icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoiceOption {
    pub label: &'static str,
    pub desc: &'static str,
    pub icon: &'static str,
    /// Bypass is the one warning option: it gets the warning colors.
    pub warn: bool,
}

pub fn permission_option(mode: PermissionMode) -> ChoiceOption {
    let (desc, icon) = match mode {
        PermissionMode::PlanOnly => (
            "Agents read and plan. Nothing is written until you approve the plan.",
            "eye",
        ),
        PermissionMode::AskEachWave => (
            "You approve every wave and every scope extension before agents write.",
            "shield",
        ),
        PermissionMode::AutoInsideReserved => (
            "Writes to files a task has reserved run without asking. Asking for more files needs your OK.",
            "lock",
        ),
        PermissionMode::Bypass => (
            "No approval cards. Free files are granted on request. Locks and the test-edit guard still apply.",
            "alert",
        ),
    };
    ChoiceOption {
        label: mode.label(),
        desc,
        icon,
        warn: mode == PermissionMode::Bypass,
    }
}

pub fn brain_option(mode: BrainMode) -> ChoiceOption {
    let (label, desc) = match mode {
        BrainMode::Off => ("Off", "No notes read or written."),
        BrainMode::ReadOnly => (
            "Read only",
            "Agents read notes. Nothing is written. Best for live runs.",
        ),
        BrainMode::Learn => (
            "Learn",
            "Agents read notes. Each run writes a journal and lesson candidates.",
        ),
    };
    ChoiceOption {
        label,
        desc,
        icon: "brain",
        warn: false,
    }
}

/// A copy of the routing with one tier set. The other tiers stay as they were.
pub fn set_tier(routing: &Routing, tier: Tier, tool: Option<ToolId>) -> Routing {
    let mut next = routing.clone();
    match tier {
        Tier::Cheap => next.cheap = tool,
        Tier::Mid => next.mid = tool,
        Tier::Smart => next.smart = tool,
    }
    next
}

/// The agents a tier can pick: enabled and ready only.
pub fn ready_tools(tools: &[ToolInfo]) -> Vec<ToolInfo> {
    tools
        .iter()
        .filter(|tool| tool.enabled && tool.status.is_ready())
        .cloned()
        .collect()
}

fn report_failure(ui: Ui, error: &api::ApiFailure) {
    ui.show_toast(error.to_string(), ToastKind::Danger);
}

fn pick_mode(ui: Ui, mode: PermissionMode) {
    if mode == PermissionMode::Bypass {
        // The confirm dialog states what stays enforced before anything changes.
        ui.modal.set(Some(Modal::BypassConfirm));
        return;
    }
    spawn_local(async move {
        let body = PermissionsUpdate {
            mode: Some(mode),
            capabilities: None,
        };
        match api::send::<_, serde_json::Value>("PUT", "/api/permissions", &body).await {
            Ok(_) => refresh(ui),
            Err(error) => report_failure(ui, &error),
        }
    });
}

fn pick_brain(ui: Ui, mode: BrainMode) {
    spawn_local(async move {
        match api::post::<_, serde_json::Value>("/api/brain/mode", &SetBrainMode { mode }).await {
            Ok(_) => refresh(ui),
            Err(error) => report_failure(ui, &error),
        }
    });
}

fn pick_route(ui: Ui, tier: Tier, value: String) {
    let tool = (!value.is_empty()).then(|| ToolId::new(value));
    let routing = set_tier(&ui.snapshot.with(|s| s.routing.clone()), tier, tool);
    spawn_local(async move {
        let body = RoutingUpdate { routing };
        match api::send::<_, serde_json::Value>("PUT", "/api/routing", &body).await {
            Ok(_) => refresh(ui),
            Err(error) => report_failure(ui, &error),
        }
    });
}

#[component]
pub fn BudgetStep() -> impl IntoView {
    view! {
        <h2>"Budget and permissions"</h2>
        <p class="lead">"Set the limits before the first run. You can lower the cap later, never raise it."</p>
        <CapField />
        <div class="psec" style="margin-top:1em">
            <h5>"Permission mode"</h5>
            <PermissionCards />
        </div>
        <div class="psec">
            <h5>"Brain"</h5>
            <BrainCards />
        </div>
        <div class="psec" style="margin-top:1em">
            <h5>"Which agent does what"</h5>
            <RoutingFields />
        </div>
    }
}

/// The cap field. It reads only after the daemon refused a change.
#[component]
fn CapField() -> impl IntoView {
    let wizard = crate::views::onboarding::use_wizard();
    view! {
        <div class="fld">
            <div class="l">
                <b>"Budget cap"</b>
                <span>"Hard stop. 70% stops new tasks. 95% kills workers."</span>
            </div>
            <span class="row">
                <span class="mono">"$"</span>
                <input
                    type="number"
                    step="0.1"
                    min="0.1"
                    aria-label="Budget cap"
                    prop:value=move || wizard.cap_text.get()
                    readonly=move || wizard.cap_locked.get()
                    on:input=move |ev| {
                        wizard.touched.set(true);
                        wizard.cap_text.set(event_target_value(&ev));
                    }
                />
            </span>
        </div>
    }
}

/// One option card, shaped like the prototype's radio buttons.
#[component]
fn ChoiceCard(
    option: ChoiceOption,
    selected: Signal<bool>,
    on_pick: Callback<()>,
) -> impl IntoView {
    view! {
        <button
            class=if option.warn { "warnr" } else { "" }
            aria-pressed=move || selected.get().to_string()
            on:click=move |_| on_pick.run(())
        >
            <span><Icon name=option.icon size=18 /></span>
            <div>
                <b>{option.label}</b>
                <div class="d">{option.desc}</div>
            </div>
        </button>
    }
}

#[component]
fn PermissionCards() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="radios">
            {PERMISSION_MODES
                .into_iter()
                .map(|mode| {
                    view! {
                        <ChoiceCard
                            option=permission_option(mode)
                            selected=Signal::derive(move || ui.snapshot.with(|s| s.permission_mode == mode))
                            on_pick=Callback::new(move |()| pick_mode(ui, mode))
                        />
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn BrainCards() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="radios">
            {BRAIN_MODES
                .into_iter()
                .map(|mode| {
                    view! {
                        <ChoiceCard
                            option=brain_option(mode)
                            selected=Signal::derive(move || ui.snapshot.with(|s| s.brain_mode == mode))
                            on_pick=Callback::new(move |()| pick_brain(ui, mode))
                        />
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn RoutingFields() -> impl IntoView {
    view! {
        {TIERS
            .into_iter()
            .map(|(tier, label, desc)| view! { <RoutingField tier=tier label=label desc=desc /> })
            .collect_view()}
    }
}

/// One tier and the select that picks its agent.
#[component]
fn RoutingField(tier: Tier, label: &'static str, desc: &'static str) -> impl IntoView {
    let ui = use_ui();
    let current = move || {
        ui.snapshot.with(|s| {
            s.routing
                .for_tier(tier)
                .map(|id| id.as_str().to_string())
                .unwrap_or_default()
        })
    };
    let on_change = move |ev| pick_route(ui, tier, event_target_value(&ev));
    view! {
        <div class="fld">
            <div class="l">
                <b>{label}</b>
                <span>{desc}</span>
            </div>
            <select aria-label=label prop:value=current on:change=on_change>
                <option value="">"None"</option>
                {move || {
                    ui.snapshot
                        .with(|s| ready_tools(&s.tools))
                        .into_iter()
                        .map(|tool| view! { <option value=tool.id.as_str().to_string()>{tool.name}</option> })
                        .collect_view()
                }}
            </select>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_one_tier_keeps_the_others() {
        let routing = Routing {
            cheap: Some(ToolId::new("claude")),
            mid: None,
            smart: None,
        };
        let next = set_tier(&routing, Tier::Smart, Some(ToolId::new("codex")));
        assert_eq!(next.cheap, Some(ToolId::new("claude")));
        assert_eq!(next.smart, Some(ToolId::new("codex")));
    }

    #[test]
    fn only_bypass_is_a_warning_option() {
        let warned: Vec<PermissionMode> = PERMISSION_MODES
            .into_iter()
            .filter(|mode| permission_option(*mode).warn)
            .collect();
        assert_eq!(warned, [PermissionMode::Bypass]);
    }

    #[test]
    fn every_permission_mode_has_its_prototype_description() {
        for mode in PERMISSION_MODES {
            assert!(!permission_option(mode).desc.is_empty(), "{mode:?}");
        }
    }

    #[test]
    fn brain_modes_are_labeled_as_the_prototype_labels_them() {
        assert_eq!(brain_option(BrainMode::ReadOnly).label, "Read only");
        assert_eq!(brain_option(BrainMode::Learn).label, "Learn");
    }
}
