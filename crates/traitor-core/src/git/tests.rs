use super::*;

#[test]
fn a_changed_line_that_looks_like_a_file_header_stays_in_its_hunk() {
    let diff = "diff --git a/query.sql b/query.sql\nindex 1..2 100644\n--- a/query.sql\n+++ b/query.sql\n@@ -1 +1 @@\n--- old comment\n+++ b/not-a-file\n@@ -5 +5 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff).files,
        vec![ChangedFile {
            path: "query.sql".to_string(),
            changes: vec![
                Change::Added(LineRange { first: 1, last: 1 }),
                Change::Added(LineRange { first: 5, last: 5 }),
            ],
        }]
    );
}

#[test]
fn reads_added_ranges_from_a_zero_context_diff() {
    let diff = "diff --git a/src/a.ts b/src/a.ts\nindex 1..2 100644\n--- a/src/a.ts\n+++ b/src/a.ts\n@@ -3 +3,2 @@ fn\n-old\n+new\n+++ b/not-a-path\n@@ -10,2 +11,0 @@\n-gone\n-gone\n@@ -20 +20 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff).files,
        vec![ChangedFile {
            path: "src/a.ts".to_string(),
            changes: vec![
                Change::Added(LineRange { first: 3, last: 4 }),
                Change::Deleted { after: 11 },
                Change::Added(LineRange {
                    first: 20,
                    last: 20
                }),
            ],
        }]
    );
}

#[test]
fn a_quoted_path_is_unquoted() {
    let diff = "diff --git \"a/we\\\"ird\\303\\251.ts\" \"b/we\\\"ird\\303\\251.ts\"\nindex 1..2 100644\n--- \"a/we\\\"ird\\303\\251.ts\"\n+++ \"b/we\\\"ird\\303\\251.ts\"\n@@ -1 +1 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff).files,
        vec![ChangedFile {
            path: "we\"ird\u{e9}.ts".to_string(),
            changes: vec![Change::Added(LineRange { first: 1, last: 1 })],
        }]
    );
}

#[test]
fn hunks_under_a_header_it_cannot_read_stay_off_the_previous_file() {
    let diff = "diff --git a/a.ts b/a.ts\n--- a/a.ts\n+++ b/a.ts\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/c.ts c/c.ts\n--- a/c.ts\n+++ c/c.ts\n@@ -9 +9 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff).files,
        vec![ChangedFile {
            path: "a.ts".to_string(),
            changes: vec![Change::Added(LineRange { first: 1, last: 1 })],
        }]
    );
}

#[test]
fn the_tab_git_adds_after_a_path_with_a_space_is_not_part_of_it() {
    let diff = "diff --git a/my file.ts b/my file.ts\n--- a/my file.ts\t\n+++ b/my file.ts\t\n@@ -1 +1 @@\n-x\n+y\ndiff --git \"a/sp \\\"q.ts\" \"b/sp \\\"q.ts\"\n--- \"a/sp \\\"q.ts\"\t\n+++ \"b/sp \\\"q.ts\"\t\n@@ -2 +2 @@\n-x\n+y\n";
    assert_eq!(
        parse_diff(diff)
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["my file.ts".to_string(), "sp \"q.ts".to_string()]
    );
}

#[test]
fn a_pure_deletion_is_read_as_a_deletion_after_its_line() {
    let diff = "diff --git a/m.py b/m.py\n--- a/m.py\n+++ b/m.py\n@@ -3,3 +2,0 @@\n-def guard(x):\n-    assert x\n-\n";
    assert_eq!(
        parse_diff(diff).files[0].changes,
        vec![Change::Deleted { after: 2 }]
    );
}

#[test]
fn an_octal_escape_takes_only_octal_digits() {
    assert_eq!(unquote("\"\\381\""), None);
    assert_eq!(unquote("\"\\303\\251\""), Some(vec![0xc3, 0xa9]));
}

#[test]
fn a_path_that_isnt_utf8_is_set_aside_rather_than_mangled() {
    let diff = "diff --git \"a/caf\\351.ts\" \"b/caf\\351.ts\"\n--- \"a/caf\\351.ts\"\n+++ \"b/caf\\351.ts\"\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/ok.ts b/ok.ts\n--- a/ok.ts\n+++ b/ok.ts\n@@ -1 +1 @@\n-x\n+y\n";
    let changed = parse_diff(diff);
    assert_eq!(
        changed
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["ok.ts".to_string()]
    );
    assert_eq!(changed.non_utf8_paths, vec!["caf\u{fffd}.ts".to_string()]);
}

#[cfg(unix)]
#[test]
fn changed_and_untracked_paths_that_arent_utf8_are_listed_apart() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let directory = crate::test_support::repository_with_trait("{}");
    let root = directory.path();
    let tracked = root.join(OsStr::from_bytes(b"tracked-\xff.ts"));
    std::fs::write(&tracked, "let a = 1\n").unwrap();
    crate::test_support::git(root, &["add", "-A"]);
    crate::test_support::git(root, &["commit", "-q", "-m", "tracked"]);
    std::fs::write(&tracked, "let a = 2\n").unwrap();
    std::fs::write(root.join(OsStr::from_bytes(b"untracked-\xfe.ts")), "x\n").unwrap();
    std::fs::write(root.join("plain.ts"), "x\n").unwrap();
    let repository = Repository::discover(root).unwrap();
    let changed = repository
        .changed_files(&repository.resolve_comparison(None).unwrap())
        .unwrap();
    assert_eq!(
        changed
            .files
            .into_iter()
            .map(|file| file.path)
            .collect::<Vec<_>>(),
        vec!["plain.ts".to_string()]
    );
    assert_eq!(
        changed.non_utf8_paths,
        vec![
            "tracked-\u{fffd}.ts".to_string(),
            "untracked-\u{fffd}.ts".to_string()
        ]
    );
}

#[test]
fn a_revision_that_looks_like_an_option_is_never_read_as_one() {
    let directory = crate::test_support::repository_with_trait("{}");
    let root = directory.path();
    let repository = Repository::discover(root).unwrap();
    for revision in ["--output=out.txt", "--output=out.txt..HEAD"] {
        let comparison = repository.resolve_comparison(Some(revision)).unwrap();
        assert!(matches!(
            repository.changed_files(&comparison),
            Err(GitError::Failed { .. })
        ));
    }
    assert!(matches!(
        repository.resolve_comparison(Some("--output=out.txt...HEAD")),
        Err(GitError::Failed { .. })
    ));
    assert!(!root.join("out.txt").exists());
}
