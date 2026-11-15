//! `traitor check`: judges changed snippets and prints what fails, as text or GitHub annotations.

use std::env;

use traitor_core::check::{Finding, Report, check};
use traitor_core::config::CONFIG_PATH;
use traitor_core::verdict::Answer;

use crate::error::warn_if_not_cached;
use crate::session::Session;
use crate::{CommandResult, Outcome};

pub(crate) async fn run(revision: Option<&str>) -> CommandResult {
    let session = Session::in_current_directory()?;
    let report = check(
        &session.repository,
        revision,
        &session.judge,
        &session.store,
    )
    .await?;

    print_report(&report);
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

    println!(
        "traitor judged {} snippets: {} fail, {} unsure, {cached} verdicts from the cache.",
        report.snippet_count,
        count(Answer::Fail),
        count(Answer::Unsure),
    );

    if !report.non_utf8_paths.is_empty() {
        println!(
            "Skipped these changed paths, which aren't valid UTF-8: {}.",
            report.non_utf8_paths.join(", ")
        );
    }

    if report.fails_are_only_reported() {
        println!(
            "No failing trait has a threshold in {CONFIG_PATH}, so its fails are reported and don't fail the check."
        );
    }
}

fn print_finding(finding: &Finding) {
    println!(
        "{}: {} at {}, p(fail) {:.2}",
        finding.verdict.answer,
        finding.definition.label(),
        finding.location(),
        finding.verdict.probabilities.fail
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

    println!(
        "::{level} file={},line={},endLine={},title={}::{}",
        escape_annotation_property(&finding.path),
        finding.snippet.first_line,
        finding.snippet.last_line,
        escape_annotation_property(&finding.definition.title),
        escape_annotation_message(&finding.definition.why)
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
