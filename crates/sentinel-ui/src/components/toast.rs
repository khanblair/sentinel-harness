//! `Toast`: one short result message, about three seconds, centered above the composer
//! (`docs/ui-tokens.md`, Toasts).

use gloo_timers::callback::Timeout;
use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::state::{Toast, ToastKind, use_ui};

const TOAST_MS: u32 = 3000;

/// The icon that shows a toast's kind. Color alone is not enough, so each kind has its own shape.
pub fn toast_icon(kind: ToastKind) -> &'static str {
    match kind {
        ToastKind::Ok => "check",
        ToastKind::Warn => "alert",
        ToastKind::Danger => "x",
        ToastKind::Info => "info",
    }
}

#[component]
pub fn Toast() -> impl IntoView {
    let ui = use_ui();
    Effect::new(move |_| {
        let Some(shown) = ui.toast.get() else {
            return;
        };
        // A toast that replaced this one must keep its own timer, so the timer only clears
        // the exact toast it was started for.
        Timeout::new(TOAST_MS, move || {
            if ui.toast.get_untracked().as_ref() == Some(&shown) {
                ui.toast.set(None);
            }
        })
        .forget();
    });
    move || ui.toast.get().map(toast_view)
}

fn toast_view(toast: Toast) -> impl IntoView {
    let class = format!("toast {}", toast.kind.class());
    let icon = toast_icon(toast.kind);
    view! {
        <div class=class role="status">
            <span class="ti"><Icon name=icon size=16 /></span>
            <span>{toast.text}</span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_toast_kind_has_its_own_class() {
        assert_eq!(ToastKind::Ok.class(), "ok");
        assert_eq!(ToastKind::Warn.class(), "warn");
        assert_eq!(ToastKind::Danger.class(), "danger");
        assert_eq!(ToastKind::Info.class(), "info");
    }

    #[test]
    fn each_toast_kind_has_its_own_icon() {
        assert_eq!(toast_icon(ToastKind::Ok), "check");
        assert_eq!(toast_icon(ToastKind::Warn), "alert");
        assert_eq!(toast_icon(ToastKind::Danger), "x");
        assert_eq!(toast_icon(ToastKind::Info), "info");
    }
}
