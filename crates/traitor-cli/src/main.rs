mod stop_hook_command;
mod test_command;

use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use semver::Version;
use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::cache::VerdictCache;
use traitor_core::check::{Finding, Report, check};
use traitor_core::config::CONFIG_PATH;
use traitor_core::init::{InitOutcome, init};
use traitor_core::verdict::Answer;

use crate::stop_hook_command::run_stop_hook;
use crate::test_command::run_test;

const EXIT_TRAIT_BROKEN: u8 = 1;
const EXIT_COULD_NOT_RUN: u8 = 2;

#[derive(Parser)]
#[command(name = "traitor", version, about)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    ClaudeCodeHook,
}

#[derive(Subcommand)]
enum Command {
    /// Set traitor up in this repo, or upgrade its config to this traitor's version. Setting up is how a repo agrees to send its code to TypeSafe.
    Init,
    /// Judge every changed snippet against the traits whose paths match it.
    Check {
        /// What to judge, as `git diff` reads it: nothing for everything unpushed, a ref for everything since it, or A..B for what changed between two commits.
        revision: Option<String>,
        /// How to report: text for people and CI, or claude-code-hook for the JSON a Claude Code Stop hook reads on stdin and prints.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Judge each trait's cases, the files in its pass/ and fail/ folders, and report any that get a different verdict.
    Test {
        /// The traits to test, by folder name. Every trait when none are given.
        traits: Vec<String>,
    },
}

fn main() -> ExitCode {
    match Arguments::parse().command {
        Command::Init => run_init(),
        Command::Check {
            revision,
            format: Format::Text,
        } => run_check(revision.as_deref()),
        Command::Check {
            revision,
            format: Format::ClaudeCodeHook,
        } => run_stop_hook(revision.as_deref()),
        Command::Test { traits } => run_test(&traits),
    }
}

fn run_init() -> ExitCode {
    let version =
        Version::parse(env!("CARGO_PKG_VERSION")).expect("cargo package versions are semver");
    let outcome = std::env::current_dir()
        .map_err(|error| error.to_string())
        .and_then(|directory| init(&directory, &version).map_err(|error| error.to_string()));
    match outcome {
        Ok(initialized) => {
            match initialized.outcome {
                InitOutcome::Created => {
                    println!("Wrote {CONFIG_PATH}.");
                    println!();
                    println!(
                        "From now on, `traitor check` sends each changed snippet in this repo to TypeSafe (api.typesafe.ai) to be judged. TypeSafe says it doesn't train on what it receives, and keeps it for as long as is reasonably necessary."
                    );
                    println!(
                        "Committing {CONFIG_PATH} is how this repo agrees to that, for everyone who works on it. Delete it to stop."
                    );
                    println!();
                    println!("Next, add a trait as .agents/traits/<slug>/TRAIT.md.");
                }
                InitOutcome::AlreadyCurrent => {
                    println!("{CONFIG_PATH} is already on traitor {version}.");
                }
                InitOutcome::Upgraded { from } => {
                    println!("Upgraded {CONFIG_PATH} from traitor {from} to {version}.");
                }
                InitOutcome::AddedSchema => {
                    println!("Pinned {CONFIG_PATH} to traitor {version}'s schema.");
                }
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("traitor couldn't set up: {message}");
            ExitCode::from(EXIT_COULD_NOT_RUN)
        }
    }
}

fn run_in_current_directory<T, E: std::fmt::Display>(
    run: impl AsyncFnOnce(&Path) -> Result<T, E>,
) -> Result<T, String> {
    let directory = std::env::current_dir().map_err(|error| error.to_string())?;
    run_in(&directory, run)
}

fn run_in<T, E: std::fmt::Display>(
    directory: &Path,
    run: impl AsyncFnOnce(&Path) -> Result<T, E>,
) -> Result<T, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime
        .block_on(run(directory))
        .map_err(|error| error.to_string())
}

fn run_check(revision: Option<&str>) -> ExitCode {
    let judge = TypeSafeJudge::from_environment();
    let cache = VerdictCache::in_user_cache_directory();
    let outcome = run_in_current_directory(async |directory: &Path| {
        check(directory, revision, &judge, &cache).await
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
            ExitCode::from(EXIT_COULD_NOT_RUN)
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
    let label = finding.verdict.answer.label();
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
