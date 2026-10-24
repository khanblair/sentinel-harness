//! Verdict tests - full truth table and adds_new_test.

use sentinel_core::TaskStatus;
use sentinel_verify::verdict::{Checks, adds_new_test, decide};

#[test]
fn decide_truth_table_all_four_true_verified() {
    let checks = Checks {
        tests_pass: true,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: true,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Verified);
    assert_eq!(v.reason, "all four checks passed");
}

#[test]
fn decide_truth_table_tamper_false_failed() {
    let checks = Checks {
        tests_pass: true,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: false,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Failed);
    assert!(v.reason.contains("tamper"));
}

#[test]
fn decide_truth_table_tests_false_failed() {
    let checks = Checks {
        tests_pass: false,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: true,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Failed);
    assert!(v.reason.contains("suite failed"));
}

#[test]
fn decide_truth_table_new_test_false_partial() {
    let checks = Checks {
        tests_pass: true,
        new_test: false,
        reviewer_ok: true,
        tamper_clear: true,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Partial);
    assert!(v.reason.contains("no new test"));
}

#[test]
fn decide_truth_table_reviewer_false_partial() {
    let checks = Checks {
        tests_pass: true,
        new_test: true,
        reviewer_ok: false,
        tamper_clear: true,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Partial);
    assert!(v.reason.contains("reviewer"));
}

#[test]
fn decide_truth_table_tests_false_and_new_test_false_failed_first() {
    // tests_pass takes priority over new_test
    let checks = Checks {
        tests_pass: false,
        new_test: false,
        reviewer_ok: true,
        tamper_clear: true,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Failed);
    assert!(v.reason.contains("suite failed"));
}

#[test]
fn decide_truth_table_tamper_false_and_tests_false_tamper_first() {
    // tamper_clear takes priority
    let checks = Checks {
        tests_pass: false,
        new_test: true,
        reviewer_ok: true,
        tamper_clear: false,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Failed);
    assert!(v.reason.contains("tamper"));
}

#[test]
fn decide_truth_table_new_test_false_and_reviewer_false_new_test_first() {
    // new_test checked before reviewer_ok
    let checks = Checks {
        tests_pass: true,
        new_test: false,
        reviewer_ok: false,
        tamper_clear: true,
    };
    let v = decide(&checks);
    assert_eq!(v.status, TaskStatus::Partial);
    assert!(v.reason.contains("no new test"));
}

#[test]
fn adds_new_test_rust_test_file() {
    let diff = r#"diff --git a/tests/foo_test.rs b/tests/foo_test.rs
new file mode 100644
--- /dev/null
+++ b/tests/foo_test.rs
@@ -0,0 +1,2 @@
+#[test]
+fn test_foo() {}
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_rust_test_attribute() {
    let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +1,3 @@
+#[test]
+fn test_new() {}
 }
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_tokio_test_attribute() {
    let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,1 +1,3 @@
+#[tokio::test]
+async fn test_async() {}
 }
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_python_test_function() {
    let diff = r#"diff --git a/test_foo.py b/test_foo.py
--- a/test_foo.py
+++ b/test_foo.py
@@ -1,1 +1,3 @@
+def test_foo():
+    assert True
 }
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_javascript_test() {
    let diff = r#"diff --git a/test.js b/test.js
--- a/test.js
+++ b/test.js
@@ -1,1 +1,3 @@
+test('foo', () => {})
+it('bar', () => {})
 }
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_go_test() {
    let diff = r#"diff --git a/foo_test.go b/foo_test.go
--- a/foo_test.go
+++ b/foo_test.go
@@ -1,1 +1,3 @@
+func TestFoo(t *testing.T) {}
 }
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_java_test() {
    let diff = r#"diff --git a/FooTest.java b/FooTest.java
--- a/FooTest.java
+++ b/FooTest.java
@@ -1,1 +1,3 @@
+@Test
+void testFoo() {}
 }
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_unrelated_diff_false() {
    let diff = r#"diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,1 +1,2 @@
 fn main() {
+    println!("hello");
 }
"#;
    assert!(!adds_new_test(diff));
}

#[test]
fn adds_new_test_header_only_false() {
    let diff = r#"diff --git a/tests/x.rs b/tests/x.rs
new file mode 100644
--- /dev/null
+++ b/tests/x.rs
@@ -0,0 +1,1 @@
+fn not_a_test() {}
"#;
    // This has "tests" in path so it SHOULD return true
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_ignores_removed_lines() {
    let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,2 @@
-#[test]
-fn old_test() {}
 fn main() {}
"#;
    assert!(!adds_new_test(diff));
}

#[test]
fn adds_new_test_spec_file_pattern() {
    let diff = r#"diff --git a/spec/foo_spec.rs b/spec/foo_spec.rs
new file mode 100644
--- /dev/null
+++ b/spec/foo_spec.rs
@@ -0,0 +1,1 @@
+fn foo() {}
"#;
    assert!(adds_new_test(diff));
}

#[test]
fn adds_new_test_test_rs_filename() {
    let diff = r#"diff --git a/src/foo_test.rs b/src/foo_test.rs
new file mode 100644
--- /dev/null
+++ b/src/foo_test.rs
@@ -0,0 +1,1 @@
+fn foo() {}
"#;
    assert!(adds_new_test(diff));
}
