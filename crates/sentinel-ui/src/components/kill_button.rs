//! `KillButton`: stops one task after a confirm that names the files it releases
//! (`docs/ui-rules.md`, Interactions).

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icon::Icon;
use crate::components::top_bar::settle;
use crate::state::{ToastKind, use_ui};

/// The confirm sentence. It names the task and the files that will be released.
pub fn confirm_text(task: &str, files: usize) -> String {
    match files {
        0 => format!("Stop {task}? It holds no reserved files."),
        1 => format!("Stop {task} and release its 1 reserved file?"),
        many => format!("Stop {task} and release its {many} reserved files?"),
    }
}

#[component]
pub fn KillButton(task: String, files: usize) -> impl IntoView {
    let ui = use_ui();
    let confirming = RwSignal::new(false);
    let question = confirm_text(&task, files);
    let task = StoredValue::new(task);
    let kill_path = StoredValue::new(format!("/api/tasks/{}/kill", task.get_value()));
    let done_text = StoredValue::new(format!("Stopped {}.", task.get_value()));
    view! {
        <Show
            when=move || confirming.get()
            fallback=move || view! {
                <button class="btn sm dng" aria-label="Kill this agent"
                    on:click=move |_| confirming.set(true)>
                    <Icon name="stop" size=13 /> " Kill"
                </button>
            }
        >
            <div class="row wrap" style="gap:.4em">
                <span class="sub">{question.clone()}</span>
                <button class="btn sm dng" on:click=move |_| {
                    confirming.set(false);
                    let path = kill_path.get_value();
                    let done = done_text.get_value();
                    spawn_local(async move {
                        let result = api::post_empty::<serde_json::Value>(&path).await;
                        if result.is_ok() {
                            ui.show_toast(done, ToastKind::Ok);
                        }
                        settle(ui, result);
                    });
                }>"Kill"</button>
                <button class="btn sm" on:click=move |_| confirming.set(false)>"Cancel"</button>
            </div>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_names_the_task_and_the_file_count() {
        assert_eq!(
            confirm_text("T-01", 1),
            "Stop T-01 and release its 1 reserved file?"
        );
        assert_eq!(
            confirm_text("T-01", 3),
            "Stop T-01 and release its 3 reserved files?"
        );
        assert_eq!(
            confirm_text("T-01", 0),
            "Stop T-01? It holds no reserved files."
        );
    }
}
