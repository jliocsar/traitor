use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::check::{Finding, Report};
use crate::store::{JsonStore, hex_sha256};
use crate::verdict::Answer;

const BLOCK: &str = "block";

#[derive(Debug, Default, Deserialize)]
pub struct StopHookInput {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct StopHookOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
}

impl StopHookOutput {
    pub fn could_not_check(message: &str) -> Self {
        Self {
            system_message: Some(format!("traitor couldn't check: {message}")),
            ..Self::default()
        }
    }

    pub fn blocks(&self) -> bool {
        self.decision.is_some()
    }
}

pub struct BlockedFails {
    store: JsonStore,
}

impl BlockedFails {
    pub fn new(store: &JsonStore) -> Self {
        Self {
            store: store.subdirectory("stop-hook"),
        }
    }

    pub fn read(&self, session_id: &str) -> BTreeSet<String> {
        self.store
            .read(&hex_sha256(session_id.as_bytes()))
            .unwrap_or_default()
    }

    pub fn write(&self, session_id: &str, fails: &BTreeSet<String>) {
        self.store
            .write_best_effort(&hex_sha256(session_id.as_bytes()), fails);
    }
}

pub fn respond(
    report: &Report,
    already_blocked: &BTreeSet<String>,
) -> (StopHookOutput, BTreeSet<String>) {
    let failing: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|finding| finding.fails_check)
        .collect();
    let failing_identities: BTreeSet<String> =
        failing.iter().map(|finding| identity(finding)).collect();
    let unsure: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|finding| finding.verdict.answer == Answer::Unsure)
        .collect();
    let output = if failing.is_empty() {
        StopHookOutput::default()
    } else if failing_identities.is_subset(already_blocked) {
        StopHookOutput {
            system_message: Some(still_failing_message(&failing)),
            ..StopHookOutput::default()
        }
    } else {
        StopHookOutput {
            decision: Some(BLOCK),
            reason: Some(block_reason(&failing, &unsure)),
            system_message: None,
        }
    };
    (output, failing_identities)
}

fn identity(finding: &Finding) -> String {
    let material = serde_json::to_vec(&(&finding.trait_slug, &finding.path, &finding.snippet.text))
        .expect("strings always serialize");
    hex_sha256(&material)
}

fn location(finding: &Finding) -> String {
    format!(
        "{}:{}-{}",
        finding.path, finding.snippet.first_line, finding.snippet.last_line
    )
}

fn block_reason(failing: &[&Finding], unsure: &[&Finding]) -> String {
    let mut reason = String::from(
        "traitor found changed code that breaks this repo's traits. Fix each one. If you think a verdict is wrong, tell the user why instead of working around it.\n",
    );
    let mut by_trait: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for finding in failing {
        by_trait
            .entry(finding.trait_slug.as_str())
            .or_default()
            .push(finding);
    }
    for (slug, findings) in by_trait {
        let first = findings[0];
        let _ = write!(
            reason,
            "\n{} ({slug})\nwhy: {}\nexpect: {}\n",
            first.trait_title, first.why, first.expect
        );
        for finding in findings {
            let _ = writeln!(
                reason,
                "- {}, p(fail) {:.2}",
                location(finding),
                finding.verdict.probabilities.fail
            );
        }
    }
    if !unsure.is_empty() {
        reason.push_str("\nJev couldn't tell for these from the snippet alone. Read them against the trait yourself:\n");
        for finding in unsure {
            let _ = writeln!(
                reason,
                "- {} ({}) at {}",
                finding.trait_title,
                finding.trait_slug,
                location(finding)
            );
        }
    }
    reason
}

fn still_failing_message(failing: &[&Finding]) -> String {
    let mut message =
        String::from("traitor: these still fail, and the agent was already told about them:");
    for finding in failing {
        let _ = write!(
            message,
            "\n- {} ({}) at {}",
            finding.trait_title,
            finding.trait_slug,
            location(finding)
        );
    }
    message
}

#[cfg(test)]
mod tests;
