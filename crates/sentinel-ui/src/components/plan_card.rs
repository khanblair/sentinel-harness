//! The plan card: the plan's counts, and the task table from the snapshot.

use leptos::prelude::*;

use crate::components::icon::Icon;
use crate::state::use_ui;

/// The flags under the plan summary. Only the non-zero counts get a line.
pub fn plan_flags(contradictions: u32, already_done: u32) -> Vec<String> {
    let mut flags = Vec::new();
    if contradictions > 0 {
        flags.push(format!(
            "{contradictions} contradicting tasks. Sentinel asks you to choose."
        ));
    }
    if already_done > 0 {
        flags.push(format!(
            "{already_done} tasks already done. They are skipped."
        ));
    }
    flags
}

#[component]
pub fn PlanCard(tasks: u32, contradictions: u32, already_done: u32) -> impl IntoView {
    let ui = use_ui();
    let flags = plan_flags(contradictions, already_done);
    view! {
        <div class="card">
            <h4><Icon name="list" size=16 />" Plan ready"</h4>
            <div class="sub">{format!("{tasks} tasks planned. Tasks in one wave never reserve the same file.")}</div>
            <ul style="margin:.5em 0 .2em 1.1em;padding:0;line-height:1.7">
                {flags.into_iter().map(|flag| view! { <li>{flag}</li> }).collect_view()}
            </ul>
            <details>
                <summary>{format!("All {tasks} tasks")}</summary>
                <table class="dl">
                    <tr><th>"Task"</th><th>"Title"</th><th>"Tier"</th><th>"Scope"</th></tr>
                    {move || ui.snapshot.with(|s| s.tasks.iter().map(|task| task_row(
                        task.id.to_string(),
                        task.title.clone(),
                        task.tier.ui_label(),
                        task.scope.entries().count(),
                    )).collect_view())}
                </table>
            </details>
        </div>
    }
}

fn task_row(id: String, title: String, tier: &'static str, scope: usize) -> impl IntoView {
    let plural = if scope == 1 { "" } else { "s" };
    view! {
        <tr>
            <td class="mono">{id}</td>
            <td>{title}</td>
            <td>{tier}</td>
            <td class="mono">{format!("{scope} path{plural}")}</td>
        </tr>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_counts_add_no_flag() {
        assert_eq!(plan_flags(0, 0), Vec::<String>::new());
    }

    #[test]
    fn contradictions_and_done_tasks_each_get_a_flag() {
        let flags = plan_flags(2, 1);
        assert_eq!(flags.len(), 2);
        assert!(flags[0].starts_with("2 contradicting tasks"));
        assert!(flags[1].starts_with("1 tasks already done"));
    }
}
