//! The fails each agent session was already told about, so it's blocked on each fail once.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::check::{Finding, Report};
use crate::store::{CacheWriteError, JsonStore, hex_sha256};

/// An agent session's id, as its harness names it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl From<String> for SessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// The fails each agent session was already blocked on, so a session is blocked on a fail once.
pub struct BlockedFails {
    store: JsonStore,
}

impl BlockedFails {
    #[must_use]
    pub fn new(store: &JsonStore) -> Self {
        Self {
            store: store.subdirectory("stop-hook"),
        }
    }

    #[must_use]
    pub fn read(&self, session: &SessionId) -> BTreeSet<String> {
        self.store
            .read(&hex_sha256(session.0.as_bytes()))
            .unwrap_or_default()
    }

    /// Remembers `fails` as the ones `session` was blocked on.
    ///
    /// # Errors
    ///
    /// When the file can't be written. The session may then be blocked on the same fails again.
    pub fn write(
        &self,
        session: &SessionId,
        fails: &BTreeSet<String>,
    ) -> Result<(), CacheWriteError> {
        self.store.write(&hex_sha256(session.0.as_bytes()), fails)
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

/// The report's fails, and whether any is new to a session already blocked on `already_blocked`.
///
/// # Errors
///
/// When a fail's identity can't be serialized to be hashed.
pub fn session_fails<'report, 'repository>(
    report: &'report Report<'repository>,
    already_blocked: &BTreeSet<String>,
) -> Result<SessionFails<'report, 'repository>, serde_json::Error> {
    let failing: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|finding| finding.fails_check())
        .collect();
    let identities: BTreeSet<String> = failing
        .iter()
        .map(|finding| identity(finding))
        .collect::<Result<_, _>>()?;
    Ok(SessionFails {
        any_new: !identities.is_subset(already_blocked),
        failing,
        identities,
    })
}

/// The same trait failing on the same snippet text has the same identity, wherever it moves.
fn identity(finding: &Finding) -> Result<String, serde_json::Error> {
    let material = serde_json::to_vec(&(
        &finding.definition.slug,
        &finding.path,
        &finding.snippet.text,
    ))?;
    Ok(hex_sha256(&material))
}

#[cfg(test)]
mod tests;
