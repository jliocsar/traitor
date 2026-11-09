use std::fs;
use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

use crate::cache::VerdictCache;
use crate::check::{CheckError, Setup, set_up};
use crate::climb::language_name;
use crate::judge::{Judge, JudgeRequest, JudgingError, judge_through_cache};
use crate::question::{Question, SnippetState};
use crate::trait_definition::{TRAITS_DIRECTORY, TraitDefinition};
use crate::verdict::{Answer, Verdict};

const CASE_DIRECTORIES: [(&str, Answer); 3] = [
    ("pass", Answer::Pass),
    ("fail", Answer::Fail),
    ("n-a", Answer::NotApplicable),
];
const CASE_FILE_STEM: &str = "case";

#[derive(Debug, Snafu)]
pub enum CaseError {
    #[snafu(transparent)]
    SetUp { source: CheckError },
    #[snafu(display("there's no {TRAITS_DIRECTORY}/{slug}"))]
    UnknownTrait { slug: String },
    #[snafu(display("couldn't read {}: {source}", path.display()))]
    ReadCase {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(transparent)]
    Judging { source: JudgingError },
}

#[derive(Debug, Clone)]
pub struct CaseResult {
    pub trait_slug: String,
    pub trait_title: String,
    pub path: String,
    pub expected: Answer,
    pub verdict: Verdict,
    pub from_cache: bool,
}

impl CaseResult {
    pub fn holds(&self) -> bool {
        self.verdict.answer == self.expected
    }
}

#[derive(Debug, Clone, Default)]
pub struct CaseReport {
    pub results: Vec<CaseResult>,
    pub traits_without_cases: Vec<String>,
}

impl CaseReport {
    pub fn all_hold(&self) -> bool {
        self.results.iter().all(CaseResult::holds)
    }
}

struct Case<'definitions> {
    definition: &'definitions TraitDefinition,
    path: String,
    expected: Answer,
    contents: String,
}

pub async fn test_cases<J: Judge>(
    directory: &Path,
    slugs: &[String],
    judge: &J,
    cache: &VerdictCache,
) -> Result<CaseReport, CaseError> {
    let Setup {
        repository,
        definitions,
        ..
    } = set_up(directory)?;
    if let Some(slug) = slugs.iter().find(|slug| {
        !definitions
            .iter()
            .any(|definition| definition.slug == **slug)
    }) {
        return UnknownTraitSnafu { slug }.fail();
    }
    let chosen: Vec<&TraitDefinition> = definitions
        .iter()
        .filter(|definition| slugs.is_empty() || slugs.contains(&definition.slug))
        .collect();

    let mut cases = Vec::new();
    let mut traits_without_cases = Vec::new();
    for definition in chosen {
        let found = cases_of(repository.root(), definition)?;
        if found.is_empty() {
            traits_without_cases.push(definition.slug.clone());
        }
        cases.extend(found);
    }

    let requests: Vec<JudgeRequest> = cases
        .iter()
        .map(|case| JudgeRequest {
            state: SnippetState {
                path: neutral_path(&case.path),
                language: language_name(&case.path),
                signature: None,
                snippet: case.contents.clone(),
            },
            questions: [(
                case.definition.slug.clone(),
                Question::about(case.definition),
            )]
            .into(),
        })
        .collect();
    let answers = judge_through_cache(judge, cache, &requests).await?;
    let results = cases
        .into_iter()
        .zip(answers)
        .filter_map(|(case, mut answers)| {
            let judged = answers.remove(&case.definition.slug)?;
            Some(CaseResult {
                trait_slug: case.definition.slug.clone(),
                trait_title: case.definition.title.clone(),
                path: case.path,
                expected: case.expected,
                verdict: judged.verdict,
                from_cache: judged.from_cache,
            })
        })
        .collect();
    Ok(CaseReport {
        results,
        traits_without_cases,
    })
}

fn cases_of<'definitions>(
    repository_root: &Path,
    definition: &'definitions TraitDefinition,
) -> Result<Vec<Case<'definitions>>, CaseError> {
    let trait_directory = Path::new(TRAITS_DIRECTORY).join(&definition.slug);
    let mut cases = Vec::new();
    for (folder, expected) in CASE_DIRECTORIES {
        let directory = repository_root.join(&trait_directory).join(folder);
        if !directory.is_dir() {
            continue;
        }
        let mut names: Vec<String> = fs::read_dir(&directory)
            .and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
            .context(ReadCaseSnafu { path: &directory })?
            .into_iter()
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.'))
            .collect();
        names.sort();
        for name in names {
            let path = directory.join(&name);
            let contents = fs::read_to_string(&path).context(ReadCaseSnafu { path: &path })?;
            cases.push(Case {
                definition,
                path: format!("{}/{folder}/{name}", trait_directory.display()),
                expected,
                contents,
            });
        }
    }
    Ok(cases)
}

fn neutral_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(
            || CASE_FILE_STEM.to_string(),
            |extension| format!("{CASE_FILE_STEM}.{extension}"),
        )
}

#[cfg(test)]
mod tests;
