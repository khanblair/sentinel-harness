//! The write-up draft, built from a fixed report and a fixed run log.

#[path = "../src/writeup.rs"]
mod writeup;

const REPORT: &str = "# Verification Report

2 of 4 tasks verified (1 Partial, 1 Blocked, 1 Skipped)

Spend: $0.42 of $2.00

| Task | Status | Why | Receipt | Cost |
|------|--------|-----|---------|------|
| T-01 | Verified | Tests pass and a second agent approved | abc12345 (reviewer) | $0.10 |
| T-02 | Partial | Tests pass, no second agent | - | $0.12 |
| T-03 | Blocked | Edited an existing test \\| refused | - | $0.20 |
| T-04 | Skipped | Cap reached | - | $0.00 |
";

const LOG_WITH_HONESTY_EVENTS: &str = r#"{"id":1,"at":"2026-10-25T09:00:00Z","event":{"kind":"RunStarted","run":"run-old","backlog":"old.md"}}
{"id":2,"at":"2026-10-25T09:05:00Z","event":{"kind":"WriteBlocked","task":"T-09","path":"src/x.rs","restored":true}}
{"id":3,"at":"2026-10-25T10:00:00Z","event":{"kind":"RunStarted","run":"run-new","backlog":"backlog.md"}}
{"id":4,"at":"2026-10-25T10:01:00Z","event":{"kind":"WriteBlocked","task":"T-02","path":"src/lib.rs","restored":true}}
{"id":5,"at":"2026-10-25T10:02:00Z","event":{"kind":"WriteBlocked","task":"T-03","path":"src/main.rs","restored":true}}
{"id":6,"at":"2026-10-25T10:03:00Z","event":{"kind":"Verdict","task":"T-02","status":"partial","reason":"Tests pass, no second agent"}}
{"id":7,"at":"2026-10-25T10:04:00Z","event":{"kind":"Verdict","task":"T-03","status":"failed","reason":"Tamper check: an existing test changed"}}
{"id":8,"at":"2026-10-25T10:05:00Z","event":{"kind":"RunFinished","verified":1,"total":4}}
{"id":9,"at":"2026-10-25T10:06:00Z","event":{"ki
"#;

const LOG_WITH_NO_HONESTY_EVENTS: &str = r#"{"id":1,"at":"2026-10-25T10:00:00Z","event":{"kind":"RunStarted","run":"run-new","backlog":"backlog.md"}}
{"id":2,"at":"2026-10-25T10:05:00Z","event":{"kind":"RunFinished","verified":2,"total":4}}
"#;

#[test]
fn the_draft_counts_the_last_run_and_lists_each_task() {
    let text = writeup::build(REPORT, LOG_WITH_HONESTY_EVENTS);
    assert!(text.starts_with(
        "# What Sentinel did\n\n1 of 4 tasks verified, 1 partial, 1 blocked, 1 skipped."
    ));
    assert!(text.contains("## Spend\n\nSpent $0.42 of $2.00."));
    assert!(
        text.contains(
            "- The write guard stopped 2 writes outside reserved files and restored them."
        )
    );
    assert!(text.contains("- 1 task edited an existing test and was failed."));
    assert!(text.contains(
        "- 1 task passed the tests but was not marked Verified because no second agent approved it."
    ));
    assert!(text.contains("- T-01, Verified: Tests pass and a second agent approved"));
    assert!(text.contains("- T-03, Blocked: Edited an existing test | refused"));
    assert!(text.ends_with(
        "Draft from `.sentinel/report.md` and `.sentinel/log.jsonl`. Edit before you share it."
    ));
}

#[test]
fn the_honesty_section_is_left_out_when_no_such_events_happened() {
    let text = writeup::build(REPORT, LOG_WITH_NO_HONESTY_EVENTS);
    assert!(text.contains("2 of 4 tasks verified"));
    assert!(!text.contains("How it stayed honest"));
}
