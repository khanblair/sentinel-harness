//! `ReservedRow`: one file path, the task that holds it, its agent and how long it has been held,
//! and the queue of waiting tasks.

use leptos::prelude::*;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::components::icon::Icon;

/// One path with its holder and waiting queue. Built by `views::reserved::group_reservations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedRowModel {
    pub path: String,
    pub holder: Option<String>,
    pub waiting: Vec<String>,
}

/// What the row adds about its holder: the agent the holder task went to, and when the file was
/// granted. Built by `views::reserved::holder_details`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolderDetail {
    pub agent: Option<String>,
    pub since: Option<OffsetDateTime>,
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

/// `for 40s`, `for 4 min`, `for 2 h`: how long a file has been held.
pub fn held_for(since: OffsetDateTime, now: OffsetDateTime) -> String {
    let seconds = (now - since).whole_seconds().max(0);
    match seconds {
        0..60 => format!("for {seconds}s"),
        60..3600 => format!("for {} min", seconds / 60),
        _ => format!("for {} h", seconds / 3600),
    }
}

/// The current time from the browser clock. `None` when the clock text does not parse.
fn now_utc() -> Option<OffsetDateTime> {
    let text = String::from(js_sys::Date::new_0().to_iso_string());
    OffsetDateTime::parse(&text, &Rfc3339).ok()
}

#[component]
pub fn ReservedRow(
    row: ReservedRowModel,
    #[prop(optional_no_strip)] detail: Option<HolderDetail>,
) -> impl IntoView {
    let ReservedRowModel {
        path,
        holder,
        waiting,
    } = row;
    let held = holder.is_some();
    let (dir, name) = split_path(&path);
    let queue = waiting_text(&waiting);
    let (agent, since) = detail.map_or((None, None), |found| (found.agent, found.since));
    let duration = since.and_then(|start| now_utc().map(|now| held_for(start, now)));
    view! {
        <div class="tr" class:held=held>
            <span class="nm"><span style="opacity:.55">{dir}</span>{name}</span>
            {holder.map(|task| view! {
                <span class="lk"><Icon name="lock" size=12 /> {task}</span>
            })}
            {agent.map(|name| view! { <span class="chip">{name}</span> })}
            {duration.map(|text| view! { <span class="sub">{text}</span> })}
            {queue.map(|text| view! { <span class="chip">{text}</span> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::Duration;

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

    #[test]
    fn held_for_picks_seconds_minutes_or_hours() {
        let now = OffsetDateTime::UNIX_EPOCH + Duration::hours(5);
        assert_eq!(held_for(now - Duration::seconds(40), now), "for 40s");
        assert_eq!(held_for(now - Duration::minutes(4), now), "for 4 min");
        assert_eq!(held_for(now - Duration::hours(2), now), "for 2 h");
    }

    #[test]
    fn held_for_never_goes_negative() {
        let now = OffsetDateTime::UNIX_EPOCH;
        assert_eq!(held_for(now + Duration::seconds(9), now), "for 0s");
    }
}
