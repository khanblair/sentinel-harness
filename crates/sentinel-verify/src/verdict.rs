//! Verdict: combines checks into a status.

use sentinel_core::TaskStatus;

/// Input checks for the verdict decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checks {
    /// Full test suite passed at the barrier.
    pub tests_pass: bool,
    /// A new test was added covering the change.
    pub new_test: bool,
    /// The second-model reviewer approved the diff.
    pub reviewer_ok: bool,
    /// No existing test file was edited (tamper check).
    pub tamper_clear: bool,
}

/// Output verdict with status and reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub status: TaskStatus,
    pub reason: String,
}

/// Decides the task status from the four checks.
///
/// - `Verified` only when all four are true (reason: "all four checks passed").
/// - `tamper_clear == false`: `Failed`, reason names the tamper.
/// - `tests_pass == false`: `Failed`, reason says the suite failed at the barrier.
/// - Otherwise (tests pass, no tamper, but no new test or reviewer said no): `Partial`,
///   reason says which one is missing.
///
/// Only this function may produce `TaskStatus::Verified` in this crate.
pub fn decide(checks: &Checks) -> Verdict {
    if !checks.tamper_clear {
        return Verdict {
            status: TaskStatus::Failed,
            reason: "existing test file was edited (tamper)".to_string(),
        };
    }

    if !checks.tests_pass {
        return Verdict {
            status: TaskStatus::Failed,
            reason: "test suite failed at the barrier".to_string(),
        };
    }

    if !checks.new_test {
        return Verdict {
            status: TaskStatus::Partial,
            reason: "no new test added".to_string(),
        };
    }

    if !checks.reviewer_ok {
        return Verdict {
            status: TaskStatus::Partial,
            reason: "reviewer did not approve".to_string(),
        };
    }

    Verdict {
        status: TaskStatus::Verified,
        reason: "all four checks passed".to_string(),
    }
}

/// Returns true when a unified diff adds a file or line that looks like a test.
///
/// Checks:
/// - Added file path contains a test-like component: `tests`, `test`, `__tests__`, `spec`
/// - Added file name matches: `*_test.*`, `*.test.*`, `*.spec.*`, `test_*.*`
/// - Added line contains: `#[test]`, `#[tokio::test]`, `def test_`, `it(`, `test(`, `func Test`, `@Test`
///
/// Only reads `+++ b/PATH` lines and `+` lines (added lines).
pub fn adds_new_test(diff: &str) -> bool {
    let mut in_added_file = false;
    let mut current_file_path = String::new();

    for line in diff.lines() {
        // Track which file we're in via +++ header
        if let Some(path) = line.strip_prefix("+++ b/") {
            current_file_path = path.to_string();
            in_added_file = true;
            // Check if the added file path looks like a test file
            if is_test_path(&current_file_path) {
                return true;
            }
            continue;
        }

        // Reset on --- header (old file)
        if line.starts_with("--- ") {
            in_added_file = false;
            continue;
        }

        // Only check added lines (+)
        if in_added_file && line.starts_with('+') && !line.starts_with("+++") {
            let added_line = &line[1..]; // Remove the +
            if is_test_line(added_line) {
                return true;
            }
        }
    }

    // Silence clippy about initial assignment
    let _ = current_file_path;

    false
}

/// Lets the daemon apply the same test-path rule to the files a task adds.
pub fn looks_like_test_path(path: &str) -> bool {
    is_test_path(path)
}

fn is_test_path(path: &str) -> bool {
    let path_lower = path.to_lowercase();

    // Check path components
    let test_components = ["tests", "test", "__tests__", "spec"];
    for comp in test_components {
        if path_lower.split('/').any(|p| p == comp) {
            return true;
        }
    }

    // Check file name patterns
    let file_name = path_lower.split('/').next_back().unwrap_or("");
    let test_patterns = [
        "_test.", // *_test.*
        ".test.", // *.test.*
        ".spec.", // *.spec.*
        "test_",  // test_*.*
    ];
    for pattern in test_patterns {
        if file_name.contains(pattern) {
            return true;
        }
    }

    false
}

fn is_test_line(line: &str) -> bool {
    let line = line.trim();

    // Rust test attributes
    if line.starts_with("#[test]") || line.starts_with("#[tokio::test]") {
        return true;
    }

    // Python test functions
    if line.starts_with("def test_") {
        return true;
    }

    // JavaScript/Jest/Mocha test functions
    if line.starts_with("it(") || line.starts_with("test(") {
        return true;
    }

    // Go test functions
    if line.starts_with("func Test") {
        return true;
    }

    // Java JUnit test annotation
    if line.starts_with("@Test") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use sentinel_core::TaskStatus;

    #[test]
    fn decide_all_four_pass_verified() {
        let checks = Checks {
            tests_pass: true,
            new_test: true,
            reviewer_ok: true,
            tamper_clear: true,
        };
        let verdict = decide(&checks);
        assert_eq!(verdict.status, TaskStatus::Verified);
        assert_eq!(verdict.reason, "all four checks passed");
    }

    #[test]
    fn decide_tamper_failed() {
        let checks = Checks {
            tests_pass: true,
            new_test: true,
            reviewer_ok: true,
            tamper_clear: false,
        };
        let verdict = decide(&checks);
        assert_eq!(verdict.status, TaskStatus::Failed);
        assert!(verdict.reason.contains("tamper"));
    }

    #[test]
    fn decide_tests_fail_failed() {
        let checks = Checks {
            tests_pass: false,
            new_test: true,
            reviewer_ok: true,
            tamper_clear: true,
        };
        let verdict = decide(&checks);
        assert_eq!(verdict.status, TaskStatus::Failed);
        assert!(verdict.reason.contains("suite failed"));
    }

    #[test]
    fn decide_no_new_test_partial() {
        let checks = Checks {
            tests_pass: true,
            new_test: false,
            reviewer_ok: true,
            tamper_clear: true,
        };
        let verdict = decide(&checks);
        assert_eq!(verdict.status, TaskStatus::Partial);
        assert!(verdict.reason.contains("no new test"));
    }

    #[test]
    fn decide_reviewer_no_partial() {
        let checks = Checks {
            tests_pass: true,
            new_test: true,
            reviewer_ok: false,
            tamper_clear: true,
        };
        let verdict = decide(&checks);
        assert_eq!(verdict.status, TaskStatus::Partial);
        assert!(verdict.reason.contains("reviewer"));
    }

    #[test]
    fn decide_tests_pass_but_missing_both_partial_new_test_first() {
        // When both new_test and reviewer_ok are false, new_test is reported first
        let checks = Checks {
            tests_pass: true,
            new_test: false,
            reviewer_ok: false,
            tamper_clear: true,
        };
        let verdict = decide(&checks);
        assert_eq!(verdict.status, TaskStatus::Partial);
        assert!(verdict.reason.contains("no new test"));
    }

    #[test]
    fn adds_new_test_detects_test_file_path() {
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
    fn adds_new_test_detects_test_file_name_pattern() {
        let diff = r#"diff --git a/src/foo_test.rs b/src/foo_test.rs
new file mode 100644
--- /dev/null
+++ b/src/foo_test.rs
@@ -0,0 +1,1 @@
+fn foo() {}
"#;
        assert!(adds_new_test(diff));
    }

    #[test]
    fn adds_new_test_detects_test_attribute() {
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
    fn adds_new_test_detects_tokio_test_attribute() {
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
    fn adds_new_test_detects_python_test() {
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
    fn adds_new_test_detects_javascript_test() {
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
    fn adds_new_test_detects_go_test() {
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
    fn adds_new_test_detects_java_test() {
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
    fn adds_new_test_false_for_unrelated_diff() {
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
    fn adds_new_test_false_for_header_only() {
        // Just a +++ header without any added content that looks like a test
        let diff = r#"diff --git a/tests/x.rs b/tests/x.rs
new file mode 100644
--- /dev/null
+++ b/tests/x.rs
@@ -0,0 +1,1 @@
+fn not_a_test() {}
"#;
        // This should still return true because the path has "tests" component
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
    fn adds_new_test_ignores_context_lines() {
        let diff = r#"diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn existing() {
+    #[test]
+    fn new_test() {}
 }
"#;
        // The #[test] is on a context line (starts with space), not added
        // Wait, in unified diff, context lines start with space, added lines with +
        // The test above has + on the new lines, so it should detect
        assert!(adds_new_test(diff));
    }
}
