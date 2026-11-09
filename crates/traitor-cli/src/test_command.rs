use std::collections::BTreeMap;
use std::env;

use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::case::{
    CaseReport, CaseResult, FAIL_CASES_DIRECTORY, PASS_CASES_DIRECTORY, test_cases,
};
use traitor_core::configured_repository::ConfiguredRepository;
use traitor_core::store::JsonStore;
use traitor_core::trait_definition::TRAITS_DIRECTORY;
use traitor_core::verdict::Answer;

use crate::{CommandResult, Outcome};

pub async fn run(traits: &[String]) -> CommandResult {
    let repository = ConfiguredRepository::open(&env::current_dir()?)?;
    let judge = TypeSafeJudge::from_environment()?;
    let store = JsonStore::in_user_cache_directory();
    let report = test_cases(&repository, traits, &judge, &store).await?;
    print_case_report(&report);
    Ok(if report.all_hold() {
        Outcome::Success
    } else {
        Outcome::TraitBroken
    })
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
            .entry(result.definition.slug.as_str())
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
            "No cases yet for {}. Add them as files in {TRAITS_DIRECTORY}/<trait>/{PASS_CASES_DIRECTORY} and {FAIL_CASES_DIRECTORY}.",
            report.traits_without_cases.join(", ")
        );
    }
}

fn fail_probability_range(results: &[&CaseResult], expected: Answer) -> String {
    let range = results
        .iter()
        .filter(|result| result.expected == expected)
        .map(|result| result.verdict.probabilities.fail)
        .fold(None, |range, fail| match range {
            None => Some((fail, fail)),
            Some((lowest, highest)) => Some((f64::min(lowest, fail), f64::max(highest, fail))),
        });
    match range {
        Some((lowest, highest)) => format!("{lowest:.2} to {highest:.2}"),
        None => "none".to_string(),
    }
}
