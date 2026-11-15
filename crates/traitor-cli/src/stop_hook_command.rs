//! `traitor check --format claude-code-hook`: the Claude Code Stop hook, which blocks an agent once on each new fail.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};
use traitor_core::blocked_fails::{BlockedFails, SessionFails, SessionId, session_fails};
use traitor_core::check::{Finding, Report, check};
use traitor_core::verdict::Answer;

use crate::error::{CliError, FailIdentitySnafu, HookOutputSnafu, error_chain, warn_if_not_cached};
use crate::session::Session;
use crate::{CommandResult, Outcome};

/// What Claude Code sends a Stop hook on stdin, as far as traitor reads it.
#[derive(Debug, Deserialize)]
struct StopHookInput {
    session_id: SessionId,
    cwd: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Decision {
    Block,
}

/// What a Stop hook prints for Claude Code: a block with its reason, a note for the user, or nothing.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
struct StopHookOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<Decision>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    system_message: Option<String>,
}

impl StopHookOutput {
    fn could_not_check(error: &dyn Error) -> Self {
        Self {
            system_message: Some(format!("traitor couldn't check: {}", error_chain(error))),
            ..Self::default()
        }
    }
}

#[derive(Debug, Snafu)]
enum InputError {
    #[snafu(display("couldn't read the Stop hook's input: {source}"))]
    Read { source: io::Error },
    #[snafu(display("the Stop hook's input isn't what Claude Code sends: {source}"))]
    Parse { source: serde_json::Error },
}

/// What Claude reads when it's blocked: each failing trait and where it fails, then the snippets
/// Jev was unsure about.
struct BlockReason<'a> {
    failing: &'a [&'a Finding<'a>],
    report: &'a Report<'a>,
}

impl fmt::Display for BlockReason<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "traitor found changed code that breaks this repo's traits. Fix each one. If you think a verdict is wrong, tell the user why instead of working around it.\n",
        )?;

        let mut by_trait: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();

        for finding in self.failing {
            by_trait
                .entry(finding.definition.slug.as_str())
                .or_default()
                .push(finding);
        }

        for findings in by_trait.into_values() {
            let definition = findings[0].definition;

            write!(
                formatter,
                "\n{}\nwhy: {}\nexpect: {}\n",
                definition.label(),
                definition.why,
                definition.expect
            )?;

            for finding in findings {
                writeln!(
                    formatter,
                    "- {}, p(fail) {:.2}",
                    finding.location(),
                    finding.verdict.probabilities.fail
                )?;
            }
        }

        let unsure: Vec<&Finding> = self
            .report
            .findings
            .iter()
            .filter(|finding| finding.verdict.answer == Answer::Unsure)
            .collect();

        if !unsure.is_empty() {
            formatter.write_str("\nJev couldn't tell for these from the snippet alone. Read them against the trait yourself:\n")?;

            for finding in unsure {
                writeln!(
                    formatter,
                    "- {} at {}",
                    finding.definition.label(),
                    finding.location()
                )?;
            }
        }

        Ok(())
    }
}

/// What the user sees when the agent stops with fails it was already blocked on.
struct StillFailing<'a> {
    failing: &'a [&'a Finding<'a>],
}

impl fmt::Display for StillFailing<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("traitor: these still fail, and the agent was already told about them:")?;

        for finding in self.failing {
            write!(
                formatter,
                "\n- {} at {}",
                finding.definition.label(),
                finding.location()
            )?;
        }

        Ok(())
    }
}

pub(crate) async fn run(revision: Option<&str>) -> CommandResult {
    let output = match read_input(io::stdin()) {
        Ok(input) => respond(&input, revision)
            .await
            .unwrap_or_else(|error| StopHookOutput::could_not_check(&error)),
        Err(error) => StopHookOutput::could_not_check(&error),
    };

    println!(
        "{}",
        serde_json::to_string(&output).context(HookOutputSnafu)?
    );

    Ok(Outcome::Success)
}

fn read_input(mut reader: impl Read) -> Result<StopHookInput, InputError> {
    let mut text = String::new();

    reader.read_to_string(&mut text).context(ReadSnafu)?;

    serde_json::from_str(&text).context(ParseSnafu)
}

async fn respond(
    input: &StopHookInput,
    revision: Option<&str>,
) -> Result<StopHookOutput, CliError> {
    let Some(session) = Session::if_it_has_traits(&input.cwd)? else {
        return Ok(StopHookOutput::default());
    };

    let report = check(
        &session.repository,
        revision,
        &session.judge,
        &session.store,
    )
    .await?;
    let blocked_fails = BlockedFails::new(&session.store);
    let fails = session_fails(&report, &blocked_fails.read(&input.session_id))
        .context(FailIdentitySnafu)?;

    warn_if_not_cached(report.cache_write_error.as_ref());

    if let Err(error) = blocked_fails.write(&input.session_id, &fails.identities) {
        eprintln!(
            "traitor couldn't remember which fails this session was told about: {}",
            error_chain(&error)
        );
    }

    Ok(output(&report, &fails))
}

fn output(report: &Report, fails: &SessionFails) -> StopHookOutput {
    if fails.failing.is_empty() {
        StopHookOutput::default()
    } else if fails.any_new {
        StopHookOutput {
            decision: Some(Decision::Block),
            reason: Some(
                BlockReason {
                    failing: &fails.failing,
                    report,
                }
                .to_string(),
            ),
            system_message: None,
        }
    } else {
        StopHookOutput {
            system_message: Some(
                StillFailing {
                    failing: &fails.failing,
                }
                .to_string(),
            ),
            ..StopHookOutput::default()
        }
    }
}

#[cfg(test)]
mod tests;
