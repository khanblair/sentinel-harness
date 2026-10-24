//! Quality notes tests - one seeded diff per rule.

use sentinel_core::ids::TaskId;
use sentinel_core::quality::{QualityKind, QualityNote};
use sentinel_verify::quality::quality_notes;

fn task_id() -> TaskId {
    TaskId::new("T-1")
}

fn find_note<'a>(notes: &'a [QualityNote], code: &str) -> Option<&'a QualityNote> {
    notes.iter().find(|n| n.detail_code == code)
}

#[test]
fn quality_long_added_block_41_lines() {
    let mut diff = String::from("+++ b/src/lib.rs\n@@ -0,0 +1,41 @@\n");
    for i in 1..=41 {
        diff.push_str(&format!("+fn foo{}() {{}}\n", i));
    }

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "long_added_block").expect("long_added_block note");
    assert_eq!(note.kind, QualityKind::Size);
    assert!(note.location.starts_with("src/lib.rs:"));
}

#[test]
fn quality_long_added_block_40_lines_no_note() {
    let mut diff = String::from("+++ b/src/lib.rs\n@@ -0,0 +1,40 @@\n");
    for i in 1..=40 {
        diff.push_str(&format!("+fn foo{}() {{}}\n", i));
    }

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "long_added_block").is_none());
}

#[test]
fn quality_long_file_past_400() {
    let diff = "+++ b/src/lib.rs\n@@ -390,20 +390,20 @@\n+fn foo() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "long_file").expect("long_file note");
    assert_eq!(note.kind, QualityKind::Size);
    assert!(note.location.starts_with("src/lib.rs:"));
}

#[test]
fn quality_long_file_under_400_no_note() {
    let diff = "+++ b/src/lib.rs\n@@ -10,20 +10,20 @@\n+fn foo() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "long_file").is_none());
}

#[test]
fn quality_duplicate_block_6_lines_60_chars() {
    let block = "+fn func_a() {}\n+fn func_b() {}\n+fn func_c() {}\n+fn func_d() {}\n+fn func_e() {}\n+fn func_f() {}\n";
    let diff = format!("+++ b/src/lib.rs\n@@ -0,0 +1,12 @@\n{}{}", block, block);

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "duplicate_block").expect("duplicate_block note");
    assert_eq!(note.kind, QualityKind::Duplication);
    assert!(note.location.starts_with("src/lib.rs:"));
}

#[test]
fn quality_duplicate_block_5_lines_no_note() {
    let block =
        "+fn func_a() {}\n+fn func_b() {}\n+fn func_c() {}\n+fn func_d() {}\n+fn func_e() {}\n";
    let diff = format!("+++ b/src/lib.rs\n@@ -0,0 +1,10 @@\n{}{}", block, block);

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "duplicate_block").is_none());
}

#[test]
fn quality_duplicate_block_less_than_60_chars_no_note() {
    let block = "+a\n+b\n+c\n+d\n+e\n+f\n";
    let diff = format!("+++ b/src/lib.rs\n@@ -0,0 +1,12 @@\n{}{}", block, block);

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "duplicate_block").is_none());
}

#[test]
fn quality_placeholder_todo_macro() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+todo!()\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "placeholder").expect("placeholder note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_placeholder_unimplemented_macro() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+unimplemented!()\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "placeholder").expect("placeholder note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_placeholder_todo_no_id() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+// TODO: fix this\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "placeholder").expect("placeholder note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_placeholder_todo_with_id_no_note() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+// TODO(P3-05): fix this\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "placeholder").is_none());
}

#[test]
fn quality_placeholder_fixme() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+// FIXME: broken\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "placeholder").expect("placeholder note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_filler_comment_matches_next_line() {
    let diff =
        "+++ b/src/lib.rs\n@@ -0,0 +1,2 @@\n+// this adds foo\n+fn add_foo() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "filler_comment").expect("filler_comment note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_filler_comment_not_triggered_on_different() {
    let diff =
        "+++ b/src/lib.rs\n@@ -0,0 +1,2 @@\n+// this adds foo\n+fn add_bar() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "filler_comment").is_none());
}

#[test]
fn quality_filler_comment_python_style() {
    let diff = "+++ b/foo.py\n@@ -0,0 +1,2 @@\n+# this adds foo\n+def add_foo():\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "filler_comment").expect("filler_comment note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_test_without_assertion_rust() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,3 @@\n+#[test]\n+fn test_foo() {}\n+}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "test_without_assertion").expect("test_without_assertion note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_test_with_assertion_no_note() {
    let diff =
        "+++ b/src/lib.rs\n@@ -0,0 +1,4 @@\n+#[test]\n+fn test_foo() {\n+    assert!(true);\n+}\n"
            .to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "test_without_assertion").is_none());
}

#[test]
fn quality_test_without_assertion_tokio_test() {
    let diff =
        "+++ b/src/lib.rs\n@@ -0,0 +1,4 @@\n+#[tokio::test]\n+async fn test_async() {}\n+}\n"
            .to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "test_without_assertion").expect("test_without_assertion note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_test_without_assertion_python() {
    let diff = "+++ b/test_foo.py\n@@ -0,0 +1,3 @@\n+def test_foo():\n+    pass\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "test_without_assertion").expect("test_without_assertion note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_test_without_assertion_javascript() {
    let diff = "+++ b/test.js\n@@ -0,0 +1,3 @@\n+test('foo', () => {})\n+}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "test_without_assertion").expect("test_without_assertion note");
    assert_eq!(note.kind, QualityKind::Slop);
}

#[test]
fn quality_test_with_expect_assertion_no_note() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,4 @@\n+#[test]\n+fn test_foo() {\n+    expect(true).to_be_true();\n+}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "test_without_assertion").is_none());
}

#[test]
fn quality_test_with_panic_assertion_no_note() {
    let diff =
        "+++ b/src/lib.rs\n@@ -0,0 +1,4 @@\n+#[test]\n+fn test_foo() {\n+    panic!();\n+}\n"
            .to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "test_without_assertion").is_none());
}

#[test]
fn quality_test_with_should_assertion_no_note() {
    let diff =
        "+++ b/test.js\n@@ -0,0 +1,3 @@\n+test('foo', () => {\n+    should.equal(1, 1);\n+})\n"
            .to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "test_without_assertion").is_none());
}

#[test]
fn quality_ignores_removed_lines() {
    let diff = "+++ b/src/lib.rs\n@@ -1,2 +1,1 @@\n-todo!()\n fn foo() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(notes.is_empty(), "notes: {:?}", notes);
}

#[test]
fn quality_ignores_context_lines() {
    // Context lines (starting with space) should not trigger notes
    let diff =
        "+++ b/src/lib.rs\n@@ -1,3 +1,3 @@\n fn existing() {\n     todo!()\n }\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(notes.is_empty(), "notes: {:?}", notes);
}

#[test]
fn quality_clean_diff_no_notes() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,2 @@\n+fn foo() {}\n+fn bar() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(notes.is_empty(), "notes: {:?}", notes);
}

#[test]
fn quality_notes_do_not_affect_decide() {
    // This test verifies that quality notes don't change the verdict
    // by checking that a diff with many notes can still be Verified
    // when all four checks pass.
    use sentinel_core::TaskStatus;
    use sentinel_verify::verdict::{Checks, decide};

    // Create a diff with many quality issues
    let mut diff = String::from("+++ b/src/lib.rs\n@@ -0,0 +1,50 @@\n");
    diff.push_str("+// TODO: fix this\n");
    for i in 1..=45 {
        diff.push_str(&format!("+fn foo{}() {{}}\n", i));
    }

    let notes = quality_notes(&task_id(), &diff);
    assert!(!notes.is_empty(), "should have quality notes");

    // But the verdict should still be Verified if all checks pass
    let checks = Checks {
        tests_pass: true,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: true,
    };
    let verdict = decide(&checks);
    assert_eq!(verdict.status, TaskStatus::Verified);
}

#[test]
fn quality_multiple_rules_in_one_diff() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,50 @@\n+// TODO: fix\n+fn a() {}\n+fn b() {}\n+fn c() {}\n+fn d() {}\n+fn e() {}\n+fn f() {}\n+fn a() {}\n+fn b() {}\n+fn c() {}\n+fn d() {}\n+fn e() {}\n+fn f() {}\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    assert!(find_note(&notes, "placeholder").is_some());
    assert!(find_note(&notes, "duplicate_block").is_some());
    assert!(find_note(&notes, "long_added_block").is_some());
}

#[test]
fn quality_location_format() {
    let diff = "+++ b/src/lib.rs\n@@ -0,0 +1,1 @@\n+todo!()\n".to_string();

    let notes = quality_notes(&task_id(), &diff);
    let note = find_note(&notes, "placeholder").expect("placeholder note");
    // Location should be path:line
    assert!(note.location.contains("src/lib.rs:"));
    let parts: Vec<&str> = note.location.split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "src/lib.rs");
    assert_eq!(parts[1].parse::<usize>().unwrap(), 1);
}

#[test]
fn quality_multiple_files() {
    let diff = r#"+++ b/src/lib.rs
@@ -0,0 +1,1 @@
+todo!()
+++ b/src/other.rs
@@ -0,0 +1,1 @@
+unimplemented!()
"#;

    let notes = quality_notes(&task_id(), diff);
    let placeholder_notes: Vec<_> = notes
        .iter()
        .filter(|n| n.detail_code == "placeholder")
        .collect();
    assert_eq!(placeholder_notes.len(), 2);
    assert!(placeholder_notes[0].location.starts_with("src/lib.rs:"));
    assert!(placeholder_notes[1].location.starts_with("src/other.rs:"));
}
