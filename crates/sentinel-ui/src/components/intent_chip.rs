//! The type line of a message in the stream: `Type: ask. REASON`, with the type as a chip. The chip
//! opens the five types. A pick sends the owner's last message again with that type.

use std::hash::{Hash, Hasher};

use leptos::prelude::*;
use sentinel_core::{Event, EventRecord, Intent};

use crate::components::icon::Icon;
use crate::state::{ToastKind, Ui, use_ui};
use crate::views::composer_send::resend_as;

/// The five types, in menu order, with the line each one shows.
pub const TYPE_ROWS: [(Intent, &str); 5] = [
    (Intent::Chat, "Small talk. Reads no file."),
    (Intent::Ask, "A question. Changes no file."),
    (
        Intent::Research,
        "Investigation with evidence. Changes no file.",
    ),
    (Intent::Plan, "A plan of tasks. Writes nothing."),
    (Intent::Change, "Changes files in the reserved files."),
];

/// A message type that a stream item can hold. `Intent` has no `Hash` and the stream keys its
/// items by hash, so the label stands in. Each type has its own label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeChoice(pub Intent);

impl Hash for TypeChoice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.label().hash(state);
    }
}

/// The chip class and icon of a type. Only a change writes files, so only it takes the accent.
pub fn chip_look(intent: Intent) -> (&'static str, &'static str) {
    match intent {
        Intent::Chat => ("chip", "spark"),
        Intent::Ask => ("chip", "book"),
        Intent::Research => ("chip", "terminal"),
        Intent::Plan => ("chip", "eye"),
        Intent::Change => ("chip a", "agent"),
    }
}

/// The owner's last message in the events. Messages from `sentinel` are its own lines, so every
/// other sender counts as the owner, phones included.
pub fn last_owner_message(events: &[EventRecord]) -> Option<String> {
    events.iter().rev().find_map(|record| {
        let Event::Message { from, text, .. } = &record.event else {
            return None;
        };
        (from != "sentinel").then(|| text.clone())
    })
}

/// The type line of one message. The chip toggles the menu of the five types.
#[component]
pub fn IntentChip(intent: Intent, reason: String) -> impl IntoView {
    let ui = use_ui();
    let open = RwSignal::new(false);
    let (class, icon) = chip_look(intent);
    let pick = Callback::new(move |picked: Intent| {
        open.set(false);
        resend_last(ui, picked);
    });
    view! {
        <div>
            <div class="note">
                <span>"Type:"</span>
                <button class=class title="Send the last message again with another type"
                    aria-expanded=move || open.get().to_string()
                    on:click=move |_| open.update(|shown| *shown = !*shown)>
                    <Icon name=icon size=13 />
                    <span>{intent.label()}</span>
                </button>
                <span>". "</span>
                <span>{reason}</span>
            </div>
            <Show when=move || open.get()>
                <TypeMenu on_pick=pick />
            </Show>
        </div>
    }
}

/// The five types. A row names its type and says what the type may do.
#[component]
fn TypeMenu(on_pick: Callback<Intent>) -> impl IntoView {
    view! {
        <div class="pop" style="position:static;max-width:none">
            {TYPE_ROWS
                .into_iter()
                .map(|(intent, desc)| view! {
                    <button class="mi" on:click=move |_| on_pick.run(intent)>
                        <span class="mono">{intent.label()}</span>
                        <span class="d">{desc}</span>
                    </button>
                })
                .collect_view()}
        </div>
    }
}

/// Sends the owner's last message of the open chat again, with `intent`.
fn resend_last(ui: Ui, intent: Intent) {
    match ui
        .snapshot
        .with_untracked(|s| last_owner_message(&s.events))
    {
        Some(text) => resend_as(ui, intent, text),
        None => ui.show_toast("There is no message to send again.", ToastKind::Warn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icons::ICONS;
    use time::OffsetDateTime;

    fn message(from: &str, text: &str) -> EventRecord {
        EventRecord {
            chat: None,
            id: 1,
            at: OffsetDateTime::UNIX_EPOCH,
            event: Event::Message {
                from: from.to_owned(),
                to: None,
                text: text.to_owned(),
            },
        }
    }

    #[test]
    fn only_a_change_takes_the_accent_chip() {
        assert_eq!(chip_look(Intent::Change).0, "chip a");
        for (intent, _) in TYPE_ROWS
            .iter()
            .filter(|(intent, _)| *intent != Intent::Change)
        {
            assert_eq!(chip_look(*intent).0, "chip");
        }
    }

    #[test]
    fn every_type_chip_uses_an_icon_from_the_set() {
        for (intent, _) in TYPE_ROWS {
            let (_, icon) = chip_look(intent);
            assert!(
                ICONS.iter().any(|(name, _)| *name == icon),
                "{icon} is missing"
            );
        }
    }

    #[test]
    fn the_menu_lists_each_type_once_in_order() {
        let labels: Vec<&str> = TYPE_ROWS.iter().map(|(intent, _)| intent.label()).collect();
        assert_eq!(labels, ["chat", "ask", "research", "plan", "change"]);
    }

    #[test]
    fn the_owner_message_is_the_last_one_that_is_not_sentinel() {
        let events = vec![
            message("Blair", "first"),
            message("Blair", "second"),
            message("sentinel", "Noted."),
        ];
        assert_eq!(last_owner_message(&events), Some("second".to_owned()));
    }

    #[test]
    fn a_phone_message_counts_as_the_owner_message() {
        let events = vec![message("Blair phone", "from the phone")];
        assert_eq!(
            last_owner_message(&events),
            Some("from the phone".to_owned())
        );
    }

    #[test]
    fn no_owner_message_gives_nothing_to_send_again() {
        let events = vec![message("sentinel", "Run started.")];
        assert_eq!(last_owner_message(&events), None);
    }
}
