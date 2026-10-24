//! `ModalHost`: the overlay frame for settings, the guide, the palette, and the bypass confirm.
//! A click on the dark backdrop closes the modal. Escape is handled by the app.

use leptos::prelude::*;
use leptos::task::spawn_local;
use sentinel_core::api::PermissionsUpdate;
use sentinel_core::PermissionMode;

use crate::api;
use crate::components::icon::Icon;
use crate::components::top_bar::settle;
use crate::state::{Modal, ToastKind, Ui, use_ui};
use crate::views::modal_stubs::{GuideModal, PaletteModal, SettingsModal};

/// The overlay id, as in `ov-<key>`. Bypass uses the key `bp`.
pub fn modal_key(modal: &Modal) -> &'static str {
    match modal {
        Modal::Settings { .. } => "settings",
        Modal::Guide => "guide",
        Modal::Palette => "palette",
        Modal::BypassConfirm => "bp",
    }
}

/// The dialog width class. The bypass confirm is a small dialog.
pub fn modal_class(modal: &Modal) -> &'static str {
    match modal {
        Modal::BypassConfirm => "modal sm",
        Modal::Settings { .. } | Modal::Guide | Modal::Palette => "modal",
    }
}

#[component]
pub fn ModalHost() -> impl IntoView {
    let ui = use_ui();
    move || {
        ui.modal.get().map(|modal| {
            let id = format!("ov-{}", modal_key(&modal));
            let class = modal_class(&modal);
            view! {
                <div class="ov" id=id on:click=move |_| ui.modal.set(None)>
                    <div class=class role="dialog" on:click=move |event| event.stop_propagation()>
                        {modal_body(&modal)}
                    </div>
                </div>
            }
        })
    }
}

fn modal_body(modal: &Modal) -> AnyView {
    match modal {
        Modal::Settings { .. } => view! { <SettingsModal /> }.into_any(),
        Modal::Guide => view! { <GuideModal /> }.into_any(),
        Modal::Palette => view! { <PaletteModal /> }.into_any(),
        Modal::BypassConfirm => view! { <BypassConfirm /> }.into_any(),
    }
}

/// Asks before the permission mode goes to Bypass, and says what stays enforced.
#[component]
pub fn BypassConfirm() -> impl IntoView {
    let ui = use_ui();
    view! {
        <div class="modal-hd">
            <span style="color:var(--warn)"><Icon name="alert" size=18 /></span>
            <h3>"Bypass permissions?"</h3>
        </div>
        <div class="modal-body" style="padding:1em 1.1em">
            <p style="margin:0 0 .7em;line-height:1.6">
                "Agents will get more files without asking, and no approval cards will appear. "
                "An agent can also run shell commands or use the network if you turned that on for it."
            </p>
            <p style="margin:0 0 1em;line-height:1.6">
                "Still enforced: a file locked by another task cannot be written, existing tests cannot be edited, and the budget cap stops the run. Best used on a repository with a clean git tree."
            </p>
            <div class="row wrap" style="justify-content:flex-end">
                <button class="btn" on:click=move |_| ui.modal.set(None)>"Cancel"</button>
                <button class="btn pri" on:click=move |_| turn_on_bypass(ui)>"Turn on Bypass permissions"</button>
            </div>
        </div>
    }
}

fn turn_on_bypass(ui: Ui) {
    ui.modal.set(None);
    let update = PermissionsUpdate {
        mode: Some(PermissionMode::Bypass),
        ..PermissionsUpdate::default()
    };
    spawn_local(async move {
        let result = api::send::<_, serde_json::Value>("PUT", "/api/permissions", &update).await;
        if result.is_ok() {
            ui.show_toast("Bypass permissions are on.", ToastKind::Warn);
        }
        settle(ui, result);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_confirm_uses_its_own_overlay_key_and_small_dialog() {
        assert_eq!(modal_key(&Modal::BypassConfirm), "bp");
        assert_eq!(modal_class(&Modal::BypassConfirm), "modal sm");
    }

    #[test]
    fn settings_guide_and_palette_use_the_full_dialog() {
        let settings = Modal::Settings {
            page: "general".to_owned(),
        };
        assert_eq!(modal_key(&settings), "settings");
        assert_eq!(modal_class(&settings), "modal");
        assert_eq!(modal_class(&Modal::Guide), "modal");
        assert_eq!(modal_key(&Modal::Palette), "palette");
    }
}
