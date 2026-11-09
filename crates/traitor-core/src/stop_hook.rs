use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snafu::Snafu;

use crate::check::{CheckError, Finding, Report, check};
use crate::configured_repository::{ConfiguredRepository, OpenError};
use crate::git::Repository;
use crate::judge::Judge;
use crate::store::{JsonStore, hex_sha256};
use crate::trait_definition::TRAITS_DIRECTORY;
use crate::verdict::Answer;

#[derive(Debug, Default, Deserialize)]
pub struct StopHookInput {
    pub session_id: Option<String>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    Block,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct StopHookOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decision>,
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
}

#[derive(Debug, Snafu)]
enum StopCheckError {
    #[snafu(transparent)]
    Open { source: OpenError },
    #[snafu(transparent)]
    Check { source: CheckError },
}

pub async fn respond_to_stop<J: Judge>(
    directory: &Path,
    session_id: &str,
    revision: Option<&str>,
    judge: &J,
    store: &JsonStore,
) -> StopHookOutput {
    let Ok(git) = Repository::discover(directory) else {
        return StopHookOutput::default();
    };
    if !git.root().join(TRAITS_DIRECTORY).is_dir() {
        return StopHookOutput::default();
    }
    check_for_stop(git, session_id, revision, judge, store)
        .await
        .unwrap_or_else(|error| StopHookOutput::could_not_check(&error.to_string()))
}

async fn check_for_stop<J: Judge>(
    git: Repository,
    session_id: &str,
    revision: Option<&str>,
    judge: &J,
    store: &JsonStore,
) -> Result<StopHookOutput, StopCheckError> {
    let repository = ConfiguredRepository::load(git)?;
    let report = check(&repository, revision, judge, store).await?;
    let blocked_fails = BlockedFails::new(store);
    let (output, now_failing) = respond(&report, &blocked_fails.read(session_id));
    blocked_fails.write(session_id, &now_failing);
    Ok(output)
}

struct BlockedFails {
    store: JsonStore,
}

impl BlockedFails {
    fn new(store: &JsonStore) -> Self {
        Self {
            store: store.subdirectory("stop-hook"),
        }
    }

    fn read(&self, session_id: &str) -> BTreeSet<String> {
        self.store
            .read(&hex_sha256(session_id.as_bytes()))
            .unwrap_or_default()
    }

    fn write(&self, session_id: &str, fails: &BTreeSet<String>) {
        self.store
            .write_best_effort(&hex_sha256(session_id.as_bytes()), fails);
    }
}

fn respond(
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
            decision: Some(Decision::Block),
            reason: Some(block_reason(&failing, &unsure)),
            system_message: None,
        }
    };
    (output, failing_identities)
}

fn identity(finding: &Finding) -> String {
    let material = serde_json::to_vec(&(
        &finding.definition.slug,
        &finding.path,
        &finding.snippet.text,
    ))
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
            .entry(finding.definition.slug.as_str())
            .or_default()
            .push(finding);
    }
    for (slug, findings) in by_trait {
        let first = findings[0];
        let _ = write!(
            reason,
            "\n{} ({slug})\nwhy: {}\nexpect: {}\n",
            first.definition.title, first.definition.why, first.definition.expect
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
                finding.definition.title,
                finding.definition.slug,
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
            finding.definition.title,
            finding.definition.slug,
            location(finding)
        );
    }
    message
}

#[cfg(test)]
mod tests;
