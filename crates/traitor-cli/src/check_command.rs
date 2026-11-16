//! `traitor check`: judges changed snippets, or whole files, and prints what fails, as text with
//! GitHub annotations or as JSON for tools.

mod json_report;

use std::env;

use snafu::{OptionExt, ResultExt};
use traitor_core::check::{Finding, Report, Scope, check};
use traitor_core::config::CONFIG_PATH;
use traitor_core::verdict::Answer;

use crate::check_command::json_report::JsonReport;
use crate::error::{CliError, JsonReportSnafu, NonUtf8RootSnafu, warn_if_not_cached};
use crate::session::Session;
use crate::{CommandResult, Outcome};

/// How `check` prints its report.
#[derive(Clone, Copy)]
pub(crate) enum Output {
    /// For people, with GitHub annotations on GitHub Actions.
    Text,
    /// For tools: the contract in `docs/check-json.md`, on one line.
    Json,
}

pub(crate) async fn run(scope: Scope<'_>, output: Output) -> CommandResult {
    let session = Session::in_current_directory()?;
    let report = check(&session.repository, scope, &session.judge, &session.store).await?;

    match output {
        Output::Text => print_report(&report),
        Output::Json => print_json(&report, &session)?,
    }

    warn_if_not_cached(report.cache_write_error.as_ref());

    Ok(if report.fails_check() {
        Outcome::TraitBroken
    } else {
        Outcome::Success
    })
}

fn print_report(report: &Report) {
    let annotate = env::var("GITHUB_ACTIONS").is_ok_and(|value| value == "true");
    let worth_reading = report.findings.iter().filter(|finding| {
        finding.fails_check() || matches!(finding.verdict.answer, Answer::Fail | Answer::Unsure)
    });

    for finding in worth_reading {
        print_finding(finding);

        if annotate {
            print_annotation(finding);
        }
    }

    let count = |answer: Answer| {
        report
            .findings
            .iter()
            .filter(|finding| finding.verdict.answer == answer)
            .count()
    };

    let cached = report
        .findings
        .iter()
        .filter(|finding| finding.from_cache)
        .count();

    let fails = count(Answer::Fail);
    let unsure = count(Answer::Unsure);

    println!(
        "traitor judged {} snippets: {fails} fail, {unsure} unsure, {cached} verdicts from the cache.",
        report.snippet_count,
    );

    if !report.non_utf8_paths.is_empty() {
        let paths = report.non_utf8_paths.join(", ");

        println!("Skipped these changed paths, which aren't valid UTF-8: {paths}.");
    }

    if report.fails_are_only_reported() {
        println!(
            "No failing trait has a threshold in {CONFIG_PATH}, so its fails are reported and don't fail the check."
        );
    }
}

fn print_json(report: &Report, session: &Session) -> Result<(), CliError> {
    let root = session.repository.root();
    let shown_root = root.to_str().context(NonUtf8RootSnafu { root })?;
    let json =
        serde_json::to_string(&JsonReport::new(report, shown_root)).context(JsonReportSnafu)?;

    println!("{json}");

    Ok(())
}

fn print_finding(finding: &Finding) {
    let label = finding.definition.label();
    let location = finding.location();

    println!(
        "{}: {label} at {location}, p(fail) {:.2}",
        finding.verdict.answer, finding.verdict.probabilities.fail
    );
    println!("  why: {}", finding.definition.why);

    if finding.verdict.answer == Answer::Unsure {
        println!(
            "  Jev couldn't tell from the snippet alone. Read it against the trait yourself rather than skipping it."
        );
    }

    println!();
}

fn print_annotation(finding: &Finding) {
    let level = if finding.fails_check() {
        "error"
    } else {
        "warning"
    };

    let file = escape_annotation_property(&finding.path);
    let title = escape_annotation_property(&finding.definition.title);
    let message = escape_annotation_message(&finding.definition.why);

    println!(
        "::{level} file={file},line={},endLine={},title={title}::{message}",
        finding.snippet.first_line, finding.snippet.last_line
    );
}

fn escape_annotation_message(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_annotation_property(text: &str) -> String {
    escape_annotation_message(text)
        .replace(':', "%3A")
        .replace(',', "%2C")
}

#[cfg(test)]
mod tests;
