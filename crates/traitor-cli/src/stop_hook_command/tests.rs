use std::collections::BTreeSet;
use std::path::Path;
use std::sync::LazyLock;

use traitor_core::Snippet;
use traitor_core::trait_definition::TraitDefinition;
use traitor_core::verdict::{Probabilities, Verdict};

use super::*;

const TRAIT: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\npaths:\n  - \"src/**/*.ts\"\n---\n\nA computed value flows through combinators, not a reassigned let.\n";

static RAILWAY: LazyLock<TraitDefinition> = LazyLock::new(|| {
    TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), TRAIT).unwrap()
});

fn verdict(answer: Answer) -> Verdict {
    let fail = if answer == Answer::Fail { 0.97 } else { 0.01 };
    Verdict {
        answer,
        confidence: 0.9,
        probabilities: Probabilities {
            pass: 1.0 - fail,
            fail,
            not_applicable: 0.0,
            unsure: 0.0,
        },
    }
}

fn finding(path: &str, text: &str, answer: Answer, threshold: Option<f64>) -> Finding<'static> {
    Finding {
        path: path.to_string(),
        snippet: Snippet {
            first_line: 1,
            last_line: 3,
            signature: None,
            text: text.to_string(),
        },
        definition: &RAILWAY,
        verdict: verdict(answer),
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

fn respond(
    report: &Report,
    already_blocked: &BTreeSet<String>,
) -> (StopHookOutput, BTreeSet<String>) {
    let fails = session_fails(report, already_blocked);
    (output(report, &fails), fails.identities)
}

#[test]
fn reads_the_session_and_directory_claude_code_sends() {
    let input =
        read_input(&br#"{"session_id":"abc","cwd":"/work","hook_event_name":"Stop"}"#[..]).unwrap();
    assert_eq!(input.session_id, "abc");
    assert_eq!(input.cwd, Path::new("/work"));
}

#[test]
fn input_without_a_session_is_an_error_not_a_shared_default() {
    let error = read_input(&br#"{"cwd":"/work"}"#[..]).unwrap_err();
    assert!(matches!(error, InputError::Parse { .. }));
}

#[test]
fn input_that_isnt_json_says_why_it_couldnt_check() {
    let error = read_input(&b"not json"[..]).unwrap_err();
    let message = StopHookOutput::could_not_check(&error)
        .system_message
        .unwrap();
    assert!(
        message.starts_with(
            "traitor couldn't check: the Stop hook's input isn't what Claude Code sends: expected"
        ),
        "{message}"
    );
}

#[test]
fn a_fail_over_its_threshold_blocks_with_the_trait_and_where() {
    let (output, blocked) = respond(
        &report(vec![finding(
            "src/a.ts",
            "let a = 1",
            Answer::Fail,
            Some(0.5),
        )]),
        &BTreeSet::new(),
    );
    assert_eq!(output.decision, Some(Decision::Block));
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
    let failing = report(vec![finding(
        "src/a.ts",
        "let a = 1",
        Answer::Fail,
        Some(0.5),
    )]);
    let (_, blocked) = respond(&failing, &BTreeSet::new());
    let (output, _) = respond(&failing, &blocked);
    assert_eq!(output.decision, None);
    assert!(output.system_message.unwrap().contains("src/a.ts:1-3"));
}

#[test]
fn a_fail_without_a_threshold_lets_the_agent_stop() {
    let (output, blocked) = respond(
        &report(vec![finding("src/a.ts", "let a = 1", Answer::Fail, None)]),
        &BTreeSet::new(),
    );
    assert_eq!(output, StopHookOutput::default());
    assert!(blocked.is_empty());
}

#[test]
fn unsure_snippets_ride_along_with_a_block() {
    let (output, _) = respond(
        &report(vec![
            finding("src/a.ts", "let a = 1", Answer::Fail, Some(0.5)),
            finding("src/c.ts", "maybe", Answer::Unsure, None),
        ]),
        &BTreeSet::new(),
    );
    assert!(output.reason.unwrap().contains("src/c.ts:1-3"));
}

#[test]
fn the_output_is_what_a_stop_hook_reads() {
    let output = StopHookOutput {
        decision: Some(Decision::Block),
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
