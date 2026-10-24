//! A write to a file the task had not reserved. Sentinel blocks it and puts the file back.

use leptos::prelude::*;

use crate::components::icon::Icon;

pub fn blocked_copy(path: &str, restored: bool) -> String {
    if restored {
        format!(
            "{path} was not reserved for the task. Sentinel blocked the write and restored the file."
        )
    } else {
        format!(
            "{path} was not reserved for the task. Sentinel is blocking the write and restoring the file."
        )
    }
}

#[component]
pub fn BlockedWriteCard(#[prop(into)] text: String) -> impl IntoView {
    view! {
        <div class="card dangc">
            <h4 style="color:var(--danger)"><Icon name="shield" size=16 />" Write blocked"</h4>
            <div>{text}</div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restored_write_says_the_file_is_back() {
        assert!(blocked_copy("src/a.rs", true).ends_with("restored the file."));
        assert!(blocked_copy("src/a.rs", false).contains("is blocking"));
    }
}
