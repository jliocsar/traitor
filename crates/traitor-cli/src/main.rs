use std::process::ExitCode;

use clap::{Parser, Subcommand};
use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::cache::VerdictCache;
use traitor_core::check::{Finding, Report, check};
use traitor_core::config::CONFIG_PATH;
use traitor_core::verdict::Answer;

const EXIT_TRAIT_BROKEN: u8 = 1;
const EXIT_COULD_NOT_CHECK: u8 = 2;

#[derive(Parser)]
#[command(name = "traitor", version, about)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Judge every changed snippet against the traits whose paths match it.
    Check {
        /// What to judge, as `git diff` reads it: nothing for everything unpushed, a ref for everything since it, or A..B for what changed between two commits.
        revision: Option<String>,
    },
}

fn main() -> ExitCode {
    match Arguments::parse().command {
        Command::Check { revision } => run_check(revision.as_deref()),
    }
}

fn run_check(revision: Option<&str>) -> ExitCode {
    let outcome = std::env::current_dir()
        .map_err(|error| error.to_string())
        .and_then(|directory| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            runtime
                .block_on(check(
                    &directory,
                    revision,
                    &TypeSafeJudge::from_environment(),
                    &VerdictCache::in_user_cache_directory(),
                ))
                .map_err(|error| error.to_string())
        });
    match outcome {
        Ok(report) => {
            print_report(&report);
            if report.fails_check() {
                ExitCode::from(EXIT_TRAIT_BROKEN)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(message) => {
            eprintln!("traitor couldn't check: {message}");
            ExitCode::from(EXIT_COULD_NOT_CHECK)
        }
    }
}

fn print_report(report: &Report) {
    let annotate = std::env::var("GITHUB_ACTIONS").is_ok_and(|value| value == "true");
    let worth_reading: Vec<&Finding> = report
        .findings
        .iter()
        .filter(|finding| {
            finding.fails_check || matches!(finding.verdict.answer, Answer::Fail | Answer::Unsure)
        })
        .collect();
    for finding in &worth_reading {
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
        "traitor judged {} snippets: {} fail, {} unsure, {} verdicts from the cache.",
        report.snippet_count,
        count(Answer::Fail),
        count(Answer::Unsure),
        cached
    );
    if count(Answer::Fail) > 0 && !report.fails_check() {
        println!(
            "No failing trait has a threshold in {CONFIG_PATH}, so its fails are reported and don't fail the check."
        );
    }
}

fn print_finding(finding: &Finding) {
    let label = match finding.verdict.answer {
        Answer::Pass => "pass",
        Answer::Fail => "fail",
        Answer::NotApplicable => "n/a",
        Answer::Unsure => "unsure",
    };
    println!(
        "{label}: {} ({}) at {}:{}-{}, p(fail) {:.2}",
        finding.trait_title,
        finding.trait_slug,
        finding.path,
        finding.snippet.first_line,
        finding.snippet.last_line,
        finding.verdict.probabilities.fail
    );
    println!("  why: {}", finding.why);
    if finding.verdict.answer == Answer::Unsure {
        println!(
            "  Jev couldn't tell from the snippet alone. Read it against the trait yourself rather than skipping it."
        );
    }
    println!();
}

fn print_annotation(finding: &Finding) {
    let level = if finding.fails_check {
        "error"
    } else {
        "warning"
    };
    println!(
        "::{level} file={},line={},endLine={},title={}::{}",
        finding.path,
        finding.snippet.first_line,
        finding.snippet.last_line,
        finding.trait_title.replace(',', "%2C"),
        finding.why
    );
}
