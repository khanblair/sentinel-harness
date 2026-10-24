//! `StatusBadge`: a shape, a color, and a label together, so state is never color alone
//! (`docs/ui-tokens.md`, Status Mapping). The shapes are the prototype's `shape()` function.

use leptos::prelude::*;
use sentinel_core::TaskStatus;

/// Color token and shape name for a status.
pub struct StatusLook {
    pub label: &'static str,
    pub color: &'static str,
    pub shape: &'static str,
}

pub fn look(status: TaskStatus) -> StatusLook {
    let (label, color, shape) = match status {
        TaskStatus::Planned | TaskStatus::Ready => ("Ready", "muted", "ring"),
        TaskStatus::Waiting => ("Waiting", "lock", "square"),
        TaskStatus::Reserved | TaskStatus::Running => ("Running", "info", "tri"),
        TaskStatus::Checkpointed | TaskStatus::Verifying => ("Verifying", "warn", "diamond"),
        TaskStatus::Verified => ("Verified", "ok", "dot"),
        TaskStatus::Failed | TaskStatus::Blocked => ("Blocked", "danger", "oct"),
        TaskStatus::Contradiction => ("Contradiction", "danger", "split"),
        TaskStatus::Skipped => ("Skipped", "muted", "dash"),
        TaskStatus::Partial => ("Partial", "warn", "half"),
    };
    StatusLook { label, color, shape }
}

/// The inner SVG markup of one shape in a 12 by 12 box.
pub fn shape_markup(shape: &str, color: &str) -> String {
    let col = format!("var(--{color})");
    match shape {
        "ring" => format!(r#"<circle cx="6" cy="6" r="4.2" fill="none" stroke="{col}" stroke-width="1.6"/>"#),
        "dot" => format!(r#"<circle cx="6" cy="6" r="4.6" fill="{col}"/>"#),
        "square" => format!(r#"<rect x="1.8" y="1.8" width="8.4" height="8.4" rx="1.2" fill="{col}"/>"#),
        "tri" => format!(r#"<polygon points="6,1.4 11,10.6 1,10.6" fill="{col}"/>"#),
        "diamond" => format!(r#"<polygon points="6,1 11,6 6,11 1,6" fill="{col}"/>"#),
        "oct" => format!(r#"<polygon points="4,1 8,1 11,4 11,8 8,11 4,11 1,8 1,4" fill="{col}"/>"#),
        "split" => format!(r#"<circle cx="6" cy="6" r="4.4" fill="none" stroke="{col}" stroke-width="1.5"/><path d="M6 1.6a4.4 4.4 0 010 8.8z" fill="{col}"/>"#),
        "half" => format!(r#"<circle cx="6" cy="6" r="4.4" fill="none" stroke="{col}" stroke-width="1.5"/><path d="M1.6 6a4.4 4.4 0 008.8 0z" fill="{col}"/>"#),
        "dash" => format!(r#"<rect x="1.5" y="5" width="9" height="2" rx="1" fill="{col}"/>"#),
        _ => String::new(),
    }
}

#[component]
pub fn StatusBadge(status: TaskStatus) -> impl IntoView {
    let look = look(status);
    view! {
        <span class="stb" style=format!("color:var(--{})", look.color)>
            <svg class="shp" viewBox="0 0 12 12" aria-hidden="true" inner_html=shape_markup(look.shape, look.color)></svg>
            <span>{look.label}</span>
        </span>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_has_a_label_and_a_shape() {
        use TaskStatus::*;
        for status in [
            Planned, Ready, Waiting, Reserved, Running, Checkpointed, Verifying, Verified, Failed,
            Blocked, Contradiction, Skipped, Partial,
        ] {
            let look = look(status);
            assert!(!look.label.is_empty());
            assert!(!shape_markup(look.shape, look.color).is_empty(), "{status:?}");
        }
    }

    #[test]
    fn labels_match_the_core_labels() {
        assert_eq!(look(TaskStatus::Verified).label, TaskStatus::Verified.label());
        assert_eq!(look(TaskStatus::Partial).label, TaskStatus::Partial.label());
    }
}
