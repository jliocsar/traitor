use std::fs;
use std::sync::LazyLock;

use futures::executor::block_on;
use tempfile::TempDir;

use super::*;
use crate::check::check;
use crate::climb::Snippet;
use crate::configured_repository::ConfiguredRepository;
use crate::test_support::{FakeJudge, railway_definition, repository_with_trait, verdict};
use crate::trait_definition::TraitDefinition;
use crate::verdict::Verdict;

static RAILWAY: LazyLock<TraitDefinition> = LazyLock::new(railway_definition);

fn finding(path: &str, text: &str, verdict: Verdict, threshold: Option<f64>) -> Finding<'static> {
    Finding {
        path: path.to_string(),
        snippet: Snippet {
            first_line: 1,
            last_line: 3,
            signature: None,
            text: text.to_string(),
        },
        definition: &RAILWAY,
        verdict,
        threshold,
        from_cache: false,
    }
}

fn report(findings: Vec<Finding<'static>>) -> Report<'static> {
    Report {
        snippet_count: findings.len(),
        findings,
        non_utf8_paths: Vec::new(),
    }
}

#[test]
fn the_same_fails_are_new_only_once() {
    let failing = report(vec![finding(
        "src/a.ts",
        "let a = 1",
        verdict(true),
        Some(0.5),
    )]);
    let first = session_fails(&failing, &BTreeSet::new());
    assert!(first.any_new);
    assert_eq!(first.failing.len(), 1);
    let again = session_fails(&failing, &first.identities);
    assert!(!again.any_new);
    assert_eq!(again.failing.len(), 1);
}

#[test]
fn a_changed_fail_is_new_again() {
    let before = report(vec![finding(
        "src/a.ts",
        "let a = 1",
        verdict(true),
        Some(0.5),
    )]);
    let after = report(vec![finding(
        "src/a.ts",
        "let a = 2",
        verdict(true),
        Some(0.5),
    )]);
    let blocked = session_fails(&before, &BTreeSet::new()).identities;
    assert!(session_fails(&after, &blocked).any_new);
}

#[test]
fn fixing_one_of_two_fails_brings_nothing_new() {
    let both = report(vec![
        finding("src/a.ts", "let a = 1", verdict(true), Some(0.5)),
        finding("src/b.ts", "let b = 1", verdict(true), Some(0.5)),
    ]);
    let one = report(vec![finding(
        "src/a.ts",
        "let a = 1",
        verdict(true),
        Some(0.5),
    )]);
    let blocked = session_fails(&both, &BTreeSet::new()).identities;
    assert!(!session_fails(&one, &blocked).any_new);
}

#[test]
fn a_fail_without_a_threshold_is_not_failing() {
    let reported_only = report(vec![finding("src/a.ts", "let a = 1", verdict(true), None)]);
    let fails = session_fails(&reported_only, &BTreeSet::new());
    assert!(fails.failing.is_empty());
    assert!(!fails.any_new);
    assert!(fails.identities.is_empty());
}

#[test]
fn each_session_keeps_its_own_blocked_fails() {
    let cache = TempDir::new().unwrap();
    let blocked_fails = BlockedFails::new(&JsonStore::at(cache.path().to_path_buf()));
    let fails = BTreeSet::from(["a".to_string()]);
    blocked_fails.write("session/../odd", &fails);
    assert!(blocked_fails.read("another").is_empty());
    assert_eq!(blocked_fails.read("session/../odd"), fails);
}

#[test]
fn a_checked_session_has_new_fails_once() {
    let directory =
        repository_with_trait(r#"{ "traits": { "railway-not-guards": { "threshold": 0.5 } } }"#);
    fs::create_dir_all(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/module.ts"), "let label = ''\n").unwrap();
    let repository = ConfiguredRepository::open(directory.path()).unwrap();
    let cache = TempDir::new().unwrap();
    let store = JsonStore::at(cache.path().to_path_buf());
    let blocked_fails = BlockedFails::new(&store);
    let stop = || {
        let report = block_on(check(&repository, None, &FakeJudge::new(), &store)).unwrap();
        let fails = session_fails(&report, &blocked_fails.read("session"));
        blocked_fails.write("session", &fails.identities);
        fails.any_new
    };
    assert!(stop());
    assert!(!stop());
}
