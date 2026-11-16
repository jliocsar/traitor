//! Tests that keep the JSON report's shape and its committed schema in step.

use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use serde_json::json;
use traitor_core::Snippet;
use traitor_core::trait_definition::TraitDefinition;
use traitor_core::verdict::{Probabilities, Verdict};

use super::*;

const COMMITTED_SCHEMA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schema/check-report.schema.json"
);

const TRAIT: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\npaths:\n  - \"src/**/*.ts\"\n---\n\nA computed value flows through combinators, not a reassigned let.\n";

static RAILWAY: LazyLock<TraitDefinition> = LazyLock::new(|| {
    TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), TRAIT).unwrap()
});

fn finding(answer: Answer, fail: f64, threshold: Option<f64>) -> Finding<'static> {
    Finding {
        path: "src/config.ts".to_string(),
        snippet: Snippet {
            first_line: 12,
            last_line: 31,
            signature: None,
            text: "let label = 'untitled'".to_string(),
        },
        definition: &RAILWAY,
        verdict: Verdict {
            answer,
            confidence: 0.9,
            probabilities: Probabilities {
                pass: 1.0 - fail,
                fail,
                not_applicable: 0.0,
                unsure: 0.0,
            },
        },
        threshold,
        from_cache: false,
    }
}

fn report_json_schema() -> String {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<JsonReport>();
    let mut text = serde_json::to_string_pretty(&schema).unwrap();

    text.push('\n');

    text
}

#[test]
fn a_report_has_one_object_per_finding_with_the_contracts_names() {
    let report = Report {
        snippet_count: 2,
        findings: vec![
            finding(Answer::Fail, 0.875, Some(0.5)),
            finding(Answer::Pass, 0.125, None),
        ],
        non_utf8_paths: Vec::new(),
        cache_write_error: None,
    };
    let value = serde_json::to_value(JsonReport::new(&report, "/work/repo")).unwrap();

    assert_eq!(
        value,
        json!({
            "version": 1,
            "root": "/work/repo",
            "snippetCount": 2,
            "failsCheck": true,
            "findings": [
                {
                    "path": "src/config.ts",
                    "firstLine": 12,
                    "lastLine": 31,
                    "trait": "railway-not-guards",
                    "title": "Railway, not guards",
                    "why": "Guards hide how a value flows.",
                    "verdict": "fail",
                    "pFail": 0.875,
                    "threshold": 0.5,
                    "failsCheck": true,
                    "fromCache": false
                },
                {
                    "path": "src/config.ts",
                    "firstLine": 12,
                    "lastLine": 31,
                    "trait": "railway-not-guards",
                    "title": "Railway, not guards",
                    "why": "Guards hide how a value flows.",
                    "verdict": "pass",
                    "pFail": 0.125,
                    "threshold": null,
                    "failsCheck": false,
                    "fromCache": false
                }
            ],
            "nonUtf8Paths": []
        })
    );
}

#[test]
fn committed_schema_is_current() {
    let generated = report_json_schema();

    if std::env::var_os("TRAITOR_WRITE_SCHEMA").is_some() {
        fs::write(COMMITTED_SCHEMA, &generated).unwrap();
    }

    let committed = fs::read_to_string(COMMITTED_SCHEMA).unwrap_or_default();

    assert!(
        committed == generated,
        "schema/check-report.schema.json is out of date. Run `TRAITOR_WRITE_SCHEMA=1 cargo test -p traitor-cli committed_schema` and commit it."
    );
}
