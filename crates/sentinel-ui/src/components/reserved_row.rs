//! `ReservedRow`: one file path, the task that holds it, and the queue of waiting tasks.

use leptos::prelude::*;

use crate::components::icon::Icon;

/// One path with its holder and waiting queue. Built by `views::reserved::group_reservations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedRowModel {
    pub path: String,
    pub holder: Option<String>,
    pub waiting: Vec<String>,
}

/// Splits `src/api/a.ts` into `("src/api/", "a.ts")`. The directory part is dimmed.
pub fn split_path(path: &str) -> (String, String) {
    match path.rfind('/') {
        Some(index) => (path[..=index].to_owned(), path[index + 1..].to_owned()),
        None => (String::new(), path.to_owned()),
    }
}

/// `waiting T-02, T-03`, or nothing when no task waits.
pub fn waiting_text(waiting: &[String]) -> Option<String> {
    (!waiting.is_empty()).then(|| format!("waiting {}", waiting.join(", ")))
}

#[component]
pub fn ReservedRow(row: ReservedRowModel) -> impl IntoView {
    let ReservedRowModel {
        path,
        holder,
        waiting,
    } = row;
    let held = holder.is_some();
    let (dir, name) = split_path(&path);
    let queue = waiting_text(&waiting);
    view! {
        <div class="tr" class:held=held>
            <span class="nm"><span style="opacity:.55">{dir}</span>{name}</span>
            {holder.map(|task| view! {
                <span class="lk"><Icon name="lock" size=12 /> {task}</span>
            })}
            {queue.map(|text| view! { <span class="chip">{text}</span> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_keeps_the_directory_separator() {
        assert_eq!(
            split_path("src/api/a.ts"),
            ("src/api/".to_owned(), "a.ts".to_owned())
        );
        assert_eq!(
            split_path("README.md"),
            (String::new(), "README.md".to_owned())
        );
    }

    #[test]
    fn waiting_text_lists_the_queue() {
        assert_eq!(waiting_text(&[]), None);
        assert_eq!(
            waiting_text(&["T-02".to_owned(), "T-03".to_owned()]).as_deref(),
            Some("waiting T-02, T-03")
        );
    }
}
