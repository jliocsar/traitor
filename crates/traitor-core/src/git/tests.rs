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
                    last: 12
                },
                LineRange {
                    first: 20,
                    last: 20
                },
            ],
        }]
    );
}

#[test]
fn a_quoted_path_is_unquoted() {
    let diff = "diff --git \"a/we\\\"ird\\303\\251.ts\" \"b/we\\\"ird\\303\\251.ts\"\nindex 1..2 100644\n--- \"a/we\\\"ird\\303\\251.ts\"\n+++ \"b/we\\\"ird\\303\\251.ts\"\n@@ -1 +1 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff),
        vec![ChangedFile {
            path: "we\"ird\u{e9}.ts".to_string(),
            changes: vec![LineRange { first: 1, last: 1 }],
        }]
    );
}

#[test]
fn hunks_under_a_header_it_cannot_read_stay_off_the_previous_file() {
    let diff = "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/c.ts c/c.ts\n--- a/c.ts\n+++ c/c.ts\n@@ -9 +9 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff),
        vec![ChangedFile {
            path: "a.ts".to_string(),
            changes: vec![LineRange { first: 1, last: 1 }],
        }]
    );
}

#[test]
fn text_with_a_nul_byte_looks_binary() {
    assert!(looks_binary("\u{89}PNG\r\n\u{1a}\n\0\0\0\rIHDR"));
    assert!(!looks_binary("export const a = 1\n"));
}

#[test]
fn the_tab_git_adds_after_a_path_with_a_space_is_not_part_of_it() {
    let diff = "diff --git a/my file.ts b/my file.ts\n--- a/my file.ts\t\n+++ b/my file.ts\t\n@@ -1 +1 @@\n-x\n+y\ndiff --git \"a/sp \\\"q.ts\" \"b/sp \\\"q.ts\"\n--- \"a/sp \\\"q.ts\"\t\n+++ \"b/sp \\\"q.ts\"\t\n@@ -2 +2 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff)
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["my file.ts".to_string(), "sp \"q.ts".to_string()]
    );
}

#[test]
fn a_deletion_covers_the_lines_on_both_sides_of_it() {
    let diff = "diff --git a/m.py b/m.py\n--- a/m.py\n+++ b/m.py\n@@ -3,3 +2,0 @@\n-def guard(x):\n-    assert x\n-\n";
    assert_eq!(
        parse_diff(diff)[0].changes,
        vec![LineRange { first: 2, last: 3 }]
    );
}
