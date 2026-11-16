//! Why a command couldn't run, and how its error is printed.

use std::error::Error;
use std::io;

use snafu::{CleanedErrorText, Snafu};
use traitor_ai_typesafe::TypeSafeError;
use traitor_core::case::CaseError;
use traitor_core::check::CheckError;
use traitor_core::configured_repository::OpenError;
use traitor_core::init::InitError;
use traitor_core::store::CacheWriteError;

/// Why a command couldn't run.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub(crate) enum CliError {
    #[snafu(display("couldn't read the current directory: {source}"))]
    CurrentDirectory { source: io::Error },
    #[snafu(display("this traitor's own version isn't valid semver: {source}"))]
    OwnVersion { source: semver::Error },
    #[snafu(transparent)]
    Open { source: OpenError },
    #[snafu(transparent)]
    Judge { source: TypeSafeError },
    #[snafu(transparent)]
    Check { source: CheckError },
    #[snafu(transparent)]
    Case { source: CaseError },
    #[snafu(transparent)]
    Init { source: InitError },
    #[snafu(display("couldn't tell this session's fails apart: {source}"))]
    FailIdentity { source: serde_json::Error },
    #[snafu(display("couldn't write the Stop hook's answer: {source}"))]
    HookOutput { source: serde_json::Error },
}

/// Says on stderr that verdicts couldn't be cached. The command's result doesn't change.
pub(crate) fn warn_if_not_cached(error: Option<&CacheWriteError>) {
    if let Some(error) = error {
        let reason = error_chain(error);

        eprintln!("traitor couldn't cache verdicts: {reason}");
    }
}

/// An error and every cause under it on one line, each cause's text said once.
///
/// A library error like reqwest's often keeps the real reason (DNS, TLS, a timeout) only in
/// its source, so printing the top error alone hides why it happened.
pub(crate) fn error_chain(error: &dyn Error) -> String {
    CleanedErrorText::new(error)
        .map(|(_, text, _)| text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests;
