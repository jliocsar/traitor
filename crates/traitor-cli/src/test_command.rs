use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::cache::VerdictCache;
use traitor_core::case::{CaseReport, CaseResult, test_cases};
use traitor_core::trait_definition::TRAITS_DIRECTORY;
use traitor_core::verdict::Answer;

use crate::{EXIT_COULD_NOT_RUN, EXIT_TRAIT_BROKEN, run_in_current_directory};

pub fn run_test(traits: &[String]) -> ExitCode {
    let judge = TypeSafeJudge::from_environment();
    let cache = VerdictCache::in_user_cache_directory();
    let outcome = run_in_current_directory(async |directory: &Path| {
        test_cases(directory, traits, &judge, &cache).await
    });
    match outcome {
        Ok(report) => {
            print_case_report(&report);
            if report.all_hold() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_TRAIT_BROKEN)
            }
        }
        Err(message) => {
            eprintln!("traitor couldn't test: {message}");
            ExitCode::from(EXIT_COULD_NOT_RUN)
        }
    }
}

fn print_case_report(report: &CaseReport) {
    for result in &report.results {
        let mark = if result.holds() { "ok" } else { "MISS" };
        println!(
            "{mark}: {} expects {}, got {}, p(fail) {:.2}",
            result.path,
            result.expected.label(),
            result.verdict.answer.label(),
            result.verdict.probabilities.fail
        );
    }
    let mut by_trait: BTreeMap<&str, Vec<&CaseResult>> = BTreeMap::new();
    for result in &report.results {
        by_trait
            .entry(result.trait_slug.as_str())
            .or_default()
            .push(result);
    }
    if !by_trait.is_empty() {
        println!();
    }
    for (slug, results) in &by_trait {
        println!(
            "{slug}: p(fail) on fail cases {}, on pass cases {}",
            fail_probability_range(results, Answer::Fail),
            fail_probability_range(results, Answer::Pass)
        );
    }
    let held = report
        .results
        .iter()
        .filter(|result| result.holds())
        .count();
    let cached = report
        .results
        .iter()
        .filter(|result| result.from_cache)
        .count();
    println!();
    println!(
        "{held} of {} cases got the verdict their folder names, {cached} from the cache.",
        report.results.len()
    );
    if !report.traits_without_cases.is_empty() {
        println!(
            "No cases yet for {}. Add them as files in {TRAITS_DIRECTORY}/<trait>/pass, fail and n-a.",
            report.traits_without_cases.join(", ")
        );
    }
}

fn fail_probability_range(results: &[&CaseResult], expected: Answer) -> String {
    let probabilities: Vec<f64> = results
        .iter()
        .filter(|result| result.expected == expected)
        .map(|result| result.verdict.probabilities.fail)
        .collect();
    let lowest = probabilities.iter().copied().reduce(f64::min);
    let highest = probabilities.iter().copied().reduce(f64::max);
    match (lowest, highest) {
        (Some(lowest), Some(highest)) => format!("{lowest:.2} to {highest:.2}"),
        _ => "none".to_string(),
    }
}
