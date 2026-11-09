use super::*;

#[test]
fn a_changed_line_that_looks_like_a_file_header_stays_in_its_hunk() {
    let diff = "diff --git a/query.sql b/query.sql\nindex 1..2 100644\n--- a/query.sql\n+++ b/query.sql\n@@ -1 +1 @@\n--- old comment\n+++ b/not-a-file\n@@ -5 +5 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff),
        vec![ChangedFile {
            path: "query.sql".to_string(),
            changes: vec![
                LineRange { first: 1, last: 1 },
                LineRange { first: 5, last: 5 },
            ],
        }]
    );
}

#[test]
fn reads_added_ranges_from_a_zero_context_diff() {
    let diff = "diff --git a/src/a.ts b/src/a.ts\nindex 1..2 100644\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -3 +3,2 @@ fn\n-old\n+new\n+++ b/not-a-path\n@@ -10,2 +11,0 @@\n-gone\n-gone\n@@ -20 +20 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff),
        vec![ChangedFile {
            path: "src/a.ts".to_string(),
            changes: vec![
                LineRange { first: 3, last: 4 },
                LineRange {
                    first: 11,
                    last: 11
                },
                LineRange {
                    first: 20,
                    last: 20
                },
            ],
        }]
    );
}
