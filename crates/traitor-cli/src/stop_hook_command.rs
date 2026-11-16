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
use traitor_core::store::CacheWriteError;
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

/// What a Stop hook answers Claude Code with.
#[derive(Debug, PartialEq, Eq)]
enum StopHookAnswer {
    /// Exit 0 with this JSON on stdout: Claude may stop, and the user may get a note.
    Allow(StopHookOutput),
    /// Exit 2 with this reason on stderr, the one channel that reaches Claude from a Stop hook:
    /// Claude reads it and keeps working.
    Block(String),
}

/// The JSON a Stop hook prints when it lets Claude stop: a note for the user, or nothing.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
struct StopHookOutput {
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    system_message: Option<String>,
}

impl StopHookOutput {
    fn could_not_check(error: &dyn Error) -> Self {
        let reason = error_chain(error);

        Self::note(format!("traitor couldn't check: {reason}"))
    }

    fn note(message: String) -> Self {
        Self {
            system_message: Some(message),
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
            let label = definition.label();

            write!(
                formatter,
                "\n{label}\nwhy: {}\nexpect: {}\n",
                definition.why, definition.expect
            )?;

            for finding in findings {
                let location = finding.location();

                writeln!(
                    formatter,
                    "- {location}, p(fail) {:.2}",
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
                let label = finding.definition.label();
                let location = finding.location();

                writeln!(formatter, "- {label} at {location}")?;
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
            let label = finding.definition.label();
            let location = finding.location();

            write!(formatter, "\n- {label} at {location}")?;
        }

        Ok(())
    }
}

/// What the user sees when there are new fails, but the session couldn't be told about them
/// without blocking every later stop on the same fails.
struct UnrememberedFails<'a> {
    failing: &'a [&'a Finding<'a>],
    error: &'a CacheWriteError,
}

impl fmt::Display for UnrememberedFails<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = error_chain(self.error);

        write!(
            formatter,
            "traitor didn't block the agent: it couldn't save which fails it told this session about ({reason}), so it would block every stop on the same fails. These fail:"
        )?;

        for finding in self.failing {
            let label = finding.definition.label();
            let location = finding.location();

            write!(formatter, "\n- {label} at {location}")?;
        }

        Ok(())
    }
}

pub(crate) async fn run(revision: Option<&str>) -> CommandResult {
    let answer = match read_input(io::stdin()) {
        Ok(input) => respond(&input, revision)
            .await
            .unwrap_or_else(|error| StopHookAnswer::Allow(StopHookOutput::could_not_check(&error))),
        Err(error) => StopHookAnswer::Allow(StopHookOutput::could_not_check(&error)),
    };

    match answer {
        StopHookAnswer::Allow(output) => {
            let json = serde_json::to_string(&output).context(HookOutputSnafu)?;

            println!("{json}");

            Ok(Outcome::Success)
        }
        StopHookAnswer::Block(reason) => {
            eprintln!("{reason}");

            Ok(Outcome::BlockStop)
        }
    }
}

fn read_input(mut reader: impl Read) -> Result<StopHookInput, InputError> {
    let mut text = String::new();

    reader.read_to_string(&mut text).context(ReadSnafu)?;

    serde_json::from_str(&text).context(ParseSnafu)
}

async fn respond(
    input: &StopHookInput,
    revision: Option<&str>,
) -> Result<StopHookAnswer, CliError> {
    let Some(session) = Session::if_it_has_traits(&input.cwd)? else {
        return Ok(StopHookAnswer::Allow(StopHookOutput::default()));
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

    let remembered = blocked_fails.write(&input.session_id, &fails.identities);

    Ok(answer(&report, &fails, remembered.err().as_ref()))
}

/// Blocks on fails new to the session, unless it couldn't remember them: then every later stop
/// would see the same fails as new and block again, forever.
fn answer(
    report: &Report,
    fails: &SessionFails,
    unremembered: Option<&CacheWriteError>,
) -> StopHookAnswer {
    if fails.failing.is_empty() {
        return StopHookAnswer::Allow(StopHookOutput::default());
    }

    match (fails.any_new, unremembered) {
        (true, None) => StopHookAnswer::Block(
            BlockReason {
                failing: &fails.failing,
                report,
            }
            .to_string(),
        ),
        (true, Some(error)) => StopHookAnswer::Allow(StopHookOutput::note(
            UnrememberedFails {
                failing: &fails.failing,
                error,
            }
            .to_string(),
        )),
        (false, _) => StopHookAnswer::Allow(StopHookOutput::note(
            StillFailing {
                failing: &fails.failing,
            }
            .to_string(),
        )),
    }
}

#[cfg(test)]
mod tests;
