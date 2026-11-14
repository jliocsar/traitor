//! Tests for the request body sent to Jev and how its answers are read.

use std::path::Path;

use traitor_core::trait_definition::TraitDefinition;

use super::*;

fn definition() -> TraitDefinition {
    TraitDefinition::parse(
        "railway-not-guards",
        Path::new("TRAIT.md"),
        "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\npaths: [\"src/**/*.ts\"]\n---\nA computed value flows through combinators.\n",
    )
    .unwrap()
}

fn request(definition: &TraitDefinition) -> JudgeRequest<'_, '_> {
    JudgeRequest {
        state: SnippetState {
            path: "src/module.ts",
            language: "typescript",
            signature: None,
            snippet: "const LIMIT = 10",
        },
        traits: vec![definition],
    }
}

#[test]
fn the_request_body_is_one_choice_per_trait() {
    let definition = definition();
    insta::assert_json_snapshot!(request_body(&request(&definition)));
}

#[test]
fn reads_a_choice_answer_as_a_verdict() {
    let response: SystemOneResponse = serde_json::from_str(
        r#"{"model":"jev-1.14.0","usage":{},"answers":{"railway-not-guards":{"type":"choice","choice":"n/a","confidence":0.76,"probabilities":{"pass":0.1,"fail":0.01,"n/a":0.89,"unsure":0}}}}"#,
    )
    .unwrap();
    let verdicts = verdicts(response);

    assert_eq!(verdicts["railway-not-guards"].answer, Answer::NotApplicable);
    assert_eq!(
        verdicts["railway-not-guards"].probabilities.not_applicable,
        0.89
    );
}

#[test]
fn a_long_error_body_is_cut_on_a_character() {
    let body = "é".repeat(ERROR_BODY_CHARS + 50);
    let shown = shorten(&body);

    assert_eq!(shown.chars().count(), ERROR_BODY_CHARS + 1);
    assert!(shown.ends_with('…'));
}

#[test]
fn a_short_error_body_is_shown_whole() {
    assert_eq!(shorten(" model not found\n"), "model not found");
}
