use std::collections::BTreeSet;

use crate::check::{Finding, Report};
use crate::store::{JsonStore, hex_sha256};

/// The fails each agent session was already blocked on, so a session is blocked on a fail once.
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

/// A report's fails, against the ones its session was already blocked on.
#[derive(Debug)]
pub struct SessionFails<'report, 'repository> {
    /// Every finding that fails the check, in the report's order.
    pub failing: Vec<&'report Finding<'repository>>,
    /// Whether any failing finding isn't among the ones the session was already blocked on.
    pub any_new: bool,
    /// Every failing finding's identity: what to remember as blocked on for the session.
    pub identities: BTreeSet<String>,
}

pub fn session_fails<'report, 'repository>(
    report: &'report Report<'repository>,
    already_blocked: &BTreeSet<String>,
) -> SessionFails<'report, 'repository> {
    let failing: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|finding| finding.fails_check())
        .collect();
    let identities: BTreeSet<String> = failing.iter().map(|finding| identity(finding)).collect();
    SessionFails {
        any_new: !identities.is_subset(already_blocked),
        failing,
        identities,
    }
}

/// The same trait failing on the same snippet text has the same identity, wherever it moves.
fn identity(finding: &Finding) -> String {
    let material = serde_json::to_vec(&(
        &finding.definition.slug,
        &finding.path,
        &finding.snippet.text,
    ))
    .expect("strings always serialize");
    hex_sha256(&material)
}

#[cfg(test)]
mod tests;
