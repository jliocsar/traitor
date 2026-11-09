use std::fs;

use futures::executor::block_on;
use tempfile::TempDir;

use super::*;
use crate::test_support::{FakeJudge, repository_with_trait};

const CASES: &str = ".agents/traits/railway-not-guards";

fn write_case(root: &Path, folder: &str, name: &str, contents: &str) {
    let directory = root.join(CASES).join(folder);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join(name), contents).unwrap();
}

fn run(root: &Path, slugs: &[String], judge: &FakeJudge) -> Result<CaseReport, CaseError> {
    let cache = TempDir::new().unwrap();
    block_on(test_cases(
        root,
        slugs,
        judge,
        &VerdictCache::at(cache.path().to_path_buf()),
    ))
}

#[test]
fn each_case_is_judged_against_the_verdict_its_folder_names() {
    let directory = repository_with_trait("{}");
    let root = directory.path();
    write_case(
        root,
        "pass",
        "railway.ts",
        "const label = Option.map(name, toTitle)\n",
    );
    write_case(root, "fail", "guarded.ts", "let label = ''\n");
    write_case(root, "fail", "looks-fine.ts", "const total = sum(items)\n");
    write_case(root, "fail", ".DS_Store", "");
    let report = run(root, &[], &FakeJudge::new()).unwrap();
    assert_eq!(
        report
            .results
            .iter()
            .map(|result| (result.path.as_str(), result.holds()))
            .collect::<Vec<_>>(),
        vec![
            (".agents/traits/railway-not-guards/pass/railway.ts", true),
            (".agents/traits/railway-not-guards/fail/guarded.ts", true),
            (
                ".agents/traits/railway-not-guards/fail/looks-fine.ts",
                false
            ),
        ]
    );
    assert!(!report.all_hold());
}

#[test]
fn an_n_a_case_expects_n_a() {
    let directory = repository_with_trait("{}");
    write_case(
        directory.path(),
        "n-a",
        "schema.ts",
        "export const Shape = Schema.Struct({})\n",
    );
    let report = run(directory.path(), &[], &FakeJudge::new()).unwrap();
    assert_eq!(report.results[0].expected, Answer::NotApplicable);
    assert!(!report.results[0].holds());
}

#[test]
fn the_judge_never_sees_the_folder_or_file_name() {
    let directory = repository_with_trait("{}");
    write_case(
        directory.path(),
        "fail",
        "guarded-bad.ts",
        "let label = ''\n",
    );
    let judge = FakeJudge::new();
    run(directory.path(), &[], &judge).unwrap();
    let states = judge.states.borrow();
    assert_eq!(states[0].path, "case.ts");
    assert_eq!(states[0].language, "typescript");
}

#[test]
fn a_trait_without_cases_is_named() {
    let directory = repository_with_trait("{}");
    let report = run(directory.path(), &[], &FakeJudge::new()).unwrap();
    assert!(report.results.is_empty());
    assert_eq!(report.traits_without_cases, vec!["railway-not-guards"]);
}

#[test]
fn an_unknown_trait_is_refused() {
    let directory = repository_with_trait("{}");
    assert!(matches!(
        run(directory.path(), &["nope".to_string()], &FakeJudge::new()),
        Err(CaseError::UnknownTrait { .. })
    ));
}
