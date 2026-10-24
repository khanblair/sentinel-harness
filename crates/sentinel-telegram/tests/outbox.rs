#![allow(clippy::unwrap_used, reason = "tests fail loudly on setup errors")]

use sentinel_telegram::Button;
use sentinel_telegram::format::Priority;
use sentinel_telegram::outbox::{Delivery, Draft, Outbox, SENDS_PER_MINUTE};
use sentinel_telegram::quiet::QuietHours;

const CHAT: i64 = 99;

fn draft(text: &str) -> Draft {
    Draft {
        chat_id: CHAT,
        text: text.to_owned(),
        keyboard: Vec::new(),
        priority: Priority::Normal,
        key: None,
    }
}

fn keyed(text: &str, key: &str) -> Draft {
    Draft {
        key: Some(key.to_owned()),
        ..draft(text)
    }
}

#[test]
fn the_same_text_inside_sixty_seconds_is_a_duplicate() {
    let mut outbox = Outbox::new();

    assert!(outbox.push(draft("Wave 2 done."), 0).is_some());
    assert!(outbox.push(draft("Wave 2 done."), 59).is_none());
}

#[test]
fn the_same_text_after_sixty_seconds_is_queued_again() {
    let mut outbox = Outbox::new();
    outbox.push(draft("Wave 2 done."), 0);

    assert!(outbox.push(draft("Wave 2 done."), 60).is_some());
}

#[test]
fn a_failed_send_waits_two_seconds_then_four() {
    let mut outbox = Outbox::new();
    let id = outbox.push(draft("hello"), 0).unwrap();
    outbox.mark_failed(id, 0, None);

    assert!(outbox.next_due(1).is_none());
    assert_eq!(outbox.next_due(2).map(|item| item.id), Some(id));
    outbox.mark_failed(id, 2, None);
    assert!(outbox.next_due(5).is_none());
    assert_eq!(outbox.next_due(6).map(|item| item.id), Some(id));
}

#[test]
fn the_sixth_failure_marks_the_message_failed_and_it_is_never_due_again() {
    let mut outbox = Outbox::new();
    let id = outbox.push(draft("hello"), 0).unwrap();
    let mut now = 0;
    for _ in 0..6 {
        outbox.mark_failed(id, now, None);
        now += 1000;
    }

    assert!(outbox.item(id).unwrap().failed);
    assert_eq!(outbox.item(id).unwrap().tries, 6);
    assert!(outbox.next_due(now + 100_000).is_none());
}

#[test]
fn retry_after_replaces_the_backoff() {
    let mut outbox = Outbox::new();
    let id = outbox.push(draft("hello"), 0).unwrap();
    outbox.mark_failed(id, 0, Some(7));

    assert!(outbox.next_due(6).is_none());
    assert_eq!(outbox.next_due(7).map(|item| item.id), Some(id));
}

#[test]
fn the_rate_limit_stops_the_twenty_first_send_for_one_minute() {
    let mut outbox = Outbox::new();
    for n in 0..=SENDS_PER_MINUTE {
        let id = outbox.push(draft(&format!("message {n}")), 0).unwrap();
        if n < SENDS_PER_MINUTE {
            let message_id = i64::try_from(n).unwrap_or_default() + 1;
            outbox.mark_sent(id, message_id, 0);
        }
    }

    assert!(outbox.next_due(0).is_none());
    assert!(outbox.next_due(59).is_none());
    assert!(outbox.next_due(60).is_some());
}

#[test]
fn a_live_key_turns_the_second_message_into_an_edit_of_the_first() {
    let mut outbox = Outbox::new();
    let first = outbox.push(keyed("Run: 1 of 3", "run"), 0).unwrap();
    outbox.mark_sent(first, 500, 0);

    let second = outbox.push(keyed("Run: 2 of 3", "run"), 1).unwrap();

    assert_eq!(outbox.item(second).unwrap().delivery, Delivery::Edit(500));
}

#[test]
fn a_live_key_with_a_message_still_queued_replaces_its_text() {
    let mut outbox = Outbox::new();
    let first = outbox.push(keyed("Run: 1 of 3", "run"), 0).unwrap();

    let again = outbox.push(keyed("Run: 2 of 3", "run"), 1).unwrap();

    assert_eq!(again, first);
    assert_eq!(outbox.item(first).unwrap().text, "Run: 2 of 3");
}

#[test]
fn the_first_message_with_a_live_key_is_a_new_send() {
    let mut outbox = Outbox::new();
    let id = outbox.push(keyed("Run: 1 of 3", "run"), 0).unwrap();

    assert_eq!(outbox.item(id).unwrap().delivery, Delivery::Send);
}

#[test]
fn a_normal_message_waits_during_quiet_hours_and_an_urgent_one_goes() {
    let night = Some(QuietHours {
        start_minute: 22 * 60,
        end_minute: 7 * 60,
    });
    let mut outbox = Outbox::new();
    outbox.set_quiet_hours(night, 0);
    let at_2300 = 23 * 3600;
    let normal = outbox.push(draft("Run finished."), at_2300).unwrap();
    let urgent = outbox
        .push(
            Draft {
                priority: Priority::Urgent,
                ..draft("Approval needed.")
            },
            at_2300,
        )
        .unwrap();

    assert_eq!(outbox.next_due(at_2300).map(|item| item.id), Some(urgent));
    outbox.mark_sent(urgent, 1, at_2300);
    assert!(
        outbox.next_due(at_2300).is_none(),
        "the normal message is held"
    );
    let at_0700 = 24 * 3600 + 7 * 3600;
    assert_eq!(outbox.next_due(at_0700).map(|item| item.id), Some(normal));
}

#[test]
fn quiet_hours_use_the_local_offset() {
    let night = Some(QuietHours {
        start_minute: 22 * 60,
        end_minute: 7 * 60,
    });
    let mut outbox = Outbox::new();
    outbox.set_quiet_hours(night, 180);
    // 20:00 UTC is 23:00 in Kampala (UTC+3), so the window holds it.
    let id = outbox.push(draft("Run finished."), 20 * 3600).unwrap();

    assert!(outbox.next_due(20 * 3600).is_none());
    assert!(outbox.item(id).is_some());
}

#[test]
fn a_sent_message_leaves_the_queue() {
    let mut outbox = Outbox::new();
    let id = outbox.push(draft("hello"), 0).unwrap();
    outbox.mark_sent(id, 7, 0);

    assert!(outbox.item(id).is_none());
    assert!(outbox.next_due(0).is_none());
}

#[test]
fn keyboards_travel_with_the_message() {
    let mut outbox = Outbox::new();
    let keyboard = vec![vec![Button {
        label: "Pause".to_owned(),
        data: "tok".to_owned(),
    }]];
    let id = outbox
        .push(
            Draft {
                keyboard: keyboard.clone(),
                ..draft("Wave done.")
            },
            0,
        )
        .unwrap();

    assert_eq!(outbox.item(id).unwrap().keyboard, keyboard);
}
