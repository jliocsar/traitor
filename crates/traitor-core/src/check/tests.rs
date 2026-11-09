use std::fs;

use futures::executor::block_on;
use tempfile::TempDir;

use super::*;
use crate::judge::JudgingError;
use crate::store::JsonStore;
use crate::test_support::{FakeJudge, git, repository_with_trait};
use crate::verdict::Answer;

const MODULE: &str = "export function guarded(name: string): string {\n  let label = 'untitled'\n  if (name) {\n    label = name\n  }\n  return label\n}\n\nexport const railway = (name: string) => Option.liftPredicate(name, isNonEmpty)\n";

fn repository(config: &str) -> TempDir {
    let directory = repository_with_trait(config);
    fs::create_dir_all(directory.path().join("src")).unwrap();
    fs::write(directory.path().join("src/module.ts"), MODULE).unwrap();
    directory
}

#[test]
fn judges_each_changed_unit_and_fails_over_the_threshold() {
    let directory = repository(r#"{ "traits": { "railway-not-guards": { "threshold": 0.5 } } }"#);
    let cache = TempDir::new().unwrap();
    let judge = FakeJudge::new();
    let report = block_on(check(
        directory.path(),
        None,
        &judge,
        &VerdictCache::new(&JsonStore::at(cache.path().to_path_buf())),
    ))
    .unwrap();
    assert_eq!(report.snippet_count, 2);
    assert_eq!(judge.calls.get(), 2);
    assert_eq!(
        report
            .findings
            .iter()
            .map(|finding| (
                finding.snippet.first_line,
                finding.verdict.answer,
                finding.fails_check
            ))
            .collect::<Vec<_>>(),
        vec![(1, Answer::Fail, true), (9, Answer::Pass, false)]
    );
    assert!(report.fails_check());
}

#[test]
fn a_second_run_reads_every_verdict_from_the_cache() {
    let directory = repository("{}");
    let cache = VerdictCache::new(&JsonStore::at(TempDir::new().unwrap().keep()));
    let judge = FakeJudge::new();
    block_on(check(directory.path(), None, &judge, &cache)).unwrap();
    let report = block_on(check(directory.path(), None, &judge, &cache)).unwrap();
    assert_eq!(judge.calls.get(), 2);
    assert!(report.findings.iter().all(|finding| finding.from_cache));
}

#[test]
fn without_a_threshold_a_fail_is_only_reported() {
    let directory = repository("{}");
    let cache = TempDir::new().unwrap();
    let report = block_on(check(
        directory.path(),
        None,
        &FakeJudge::new(),
        &VerdictCache::new(&JsonStore::at(cache.path().to_path_buf())),
    ))
    .unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.verdict.answer == Answer::Fail)
    );
    assert!(!report.fails_check());
}

#[test]
fn an_exempt_path_is_never_asked_about() {
    let directory = repository(
        r#"{ "traits": { "railway-not-guards": { "exempt": [{ "paths": ["src/module.ts"], "reason": "generated" }] } } }"#,
    );
    let cache = TempDir::new().unwrap();
    let judge = FakeJudge::new();
    let report = block_on(check(
        directory.path(),
        None,
        &judge,
        &VerdictCache::new(&JsonStore::at(cache.path().to_path_buf())),
    ))
    .unwrap();
    assert_eq!(judge.calls.get(), 0);
    assert!(report.findings.is_empty());
}

#[test]
fn a_repo_without_a_config_cannot_be_checked() {
    let directory = repository("{}");
    fs::remove_file(directory.path().join(".agents/traitor.json")).unwrap();
    let cache = TempDir::new().unwrap();
    let result = block_on(check(
        directory.path(),
        None,
        &FakeJudge::new(),
        &VerdictCache::new(&JsonStore::at(cache.path().to_path_buf())),
    ));
    assert!(matches!(
        result,
        Err(CheckError::Config {
            source: ConfigError::MissingConfig { .. }
        })
    ));
}

#[test]
fn an_offline_judge_cannot_check() {
    let directory = repository("{}");
    let cache = TempDir::new().unwrap();
    let judge = FakeJudge::offline();
    let result = block_on(check(
        directory.path(),
        None,
        &judge,
        &VerdictCache::new(&JsonStore::at(cache.path().to_path_buf())),
    ));
    assert!(matches!(
        result,
        Err(CheckError::Judging {
            source: JudgingError::Judge { .. }
        })
    ));
}

#[test]
fn a_range_reads_files_from_its_last_commit() {
    let directory = repository("{}");
    git(directory.path(), &["add", "-A"]);
    git(directory.path(), &["commit", "-q", "-m", "module"]);
    fs::write(
        directory.path().join("src/module.ts"),
        "export const changed = 1\n",
    )
    .unwrap();
    let cache = TempDir::new().unwrap();
    let report = block_on(check(
        directory.path(),
        Some("HEAD~..HEAD"),
        &FakeJudge::new(),
        &VerdictCache::new(&JsonStore::at(cache.path().to_path_buf())),
    ))
    .unwrap();
    assert_eq!(report.snippet_count, 2);
    assert!(
        report
            .findings
            .iter()
            .all(|finding| !finding.snippet.text.contains("changed"))
    );
}
