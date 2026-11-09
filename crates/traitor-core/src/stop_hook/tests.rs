use std::sync::LazyLock;

use tempfile::TempDir;

use super::*;
use crate::climb::Snippet;
use crate::test_support::{railway_definition, verdict};
use crate::trait_definition::TraitDefinition;
use crate::verdict::Verdict;

static RAILWAY: LazyLock<TraitDefinition> = LazyLock::new(railway_definition);

fn finding(path: &str, text: &str, verdict: Verdict, fails_check: bool) -> Finding<'static> {
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
        fails_check,
        from_cache: false,
    }
}

fn report(findings: Vec<Finding<'static>>) -> Report<'static> {
    Report {
        snippet_count: findings.len(),
        findings,
    }
}

#[test]
fn a_fail_over_its_threshold_blocks_with_the_trait_and_where() {
    let (output, blocked) = respond(
        &report(vec![finding("src/a.ts", "let a = 1", verdict(true), true)]),
        &BTreeSet::new(),
    );
    assert!(output.blocks());
    let reason = output.reason.unwrap();
    assert!(reason.contains("Railway, not guards (railway-not-guards)\n"));
    assert!(
        reason.contains(
            "expect: A computed value flows through combinators, not a reassigned let.\n"
        )
    );
    assert!(reason.contains("- src/a.ts:1-3, p(fail) 0.97"));
    assert_eq!(blocked.len(), 1);
}

#[test]
fn the_same_fails_block_only_once() {
    let failing = report(vec![finding("src/a.ts", "let a = 1", verdict(true), true)]);
    let (_, blocked) = respond(&failing, &BTreeSet::new());
    let (output, _) = respond(&failing, &blocked);
    assert!(!output.blocks());
    assert!(output.system_message.unwrap().contains("src/a.ts:1-3"));
}

#[test]
fn a_changed_fail_blocks_again() {
    let (_, blocked) = respond(
        &report(vec![finding("src/a.ts", "let a = 1", verdict(true), true)]),
        &BTreeSet::new(),
    );
    let (output, _) = respond(
        &report(vec![finding("src/a.ts", "let a = 2", verdict(true), true)]),
        &blocked,
    );
    assert!(output.blocks());
}

#[test]
fn fixing_one_of_two_fails_does_not_block_again() {
    let (_, blocked) = respond(
        &report(vec![
            finding("src/a.ts", "let a = 1", verdict(true), true),
            finding("src/b.ts", "let b = 1", verdict(true), true),
        ]),
        &BTreeSet::new(),
    );
    let (output, _) = respond(
        &report(vec![finding("src/a.ts", "let a = 1", verdict(true), true)]),
        &blocked,
    );
    assert!(!output.blocks());
}

#[test]
fn a_fail_without_a_threshold_lets_the_agent_stop() {
    let (output, blocked) = respond(
        &report(vec![finding("src/a.ts", "let a = 1", verdict(true), false)]),
        &BTreeSet::new(),
    );
    assert_eq!(output, StopHookOutput::default());
    assert!(blocked.is_empty());
}

#[test]
fn unsure_snippets_ride_along_with_a_block() {
    let mut unsure = verdict(false);
    unsure.answer = Answer::Unsure;
    let (output, _) = respond(
        &report(vec![
            finding("src/a.ts", "let a = 1", verdict(true), true),
            finding("src/c.ts", "maybe", unsure, false),
        ]),
        &BTreeSet::new(),
    );
    assert!(output.reason.unwrap().contains("src/c.ts:1-3"));
}

#[test]
fn blocked_fails_persist_per_session() {
    let directory = TempDir::new().unwrap();
    let store = BlockedFails::new(&JsonStore::at(directory.path().to_path_buf()));
    let fails = BTreeSet::from(["one".to_string()]);
    store.write("session/../odd", &fails);
    assert_eq!(store.read("session/../odd"), fails);
    assert!(store.read("another").is_empty());
}

#[test]
fn the_output_is_what_a_stop_hook_reads() {
    let output = StopHookOutput {
        decision: Some("block"),
        reason: Some("fix it".to_string()),
        system_message: None,
    };
    assert_eq!(
        serde_json::to_string(&output).unwrap(),
        r#"{"decision":"block","reason":"fix it"}"#
    );
    assert_eq!(
        serde_json::to_string(&StopHookOutput::default()).unwrap(),
        "{}"
    );
}
