use std::fs;
use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

use crate::cache::VerdictCache;
use crate::climb::language_name;
use crate::configured_repository::ConfiguredRepository;
use crate::judge::{Judge, JudgeRequest, JudgingError, judge_through_cache};
use crate::question::SnippetState;
use crate::trait_definition::{TRAITS_DIRECTORY, TraitDefinition, visible_entries};
use crate::verdict::{Answer, Verdict};

const CASE_DIRECTORIES: [(&str, Answer); 2] = [("pass", Answer::Pass), ("fail", Answer::Fail)];
const CASE_FILE_STEM: &str = "case";

#[derive(Debug, Snafu)]
pub enum CaseError {
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
pub struct CaseResult<'repository> {
    pub definition: &'repository TraitDefinition,
    pub path: String,
    pub expected: Answer,
    pub verdict: Verdict,
    pub from_cache: bool,
}

impl CaseResult<'_> {
    pub fn holds(&self) -> bool {
        match self.expected {
            Answer::Pass => matches!(self.verdict.answer, Answer::Pass | Answer::NotApplicable),
            expected => self.verdict.answer == expected,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CaseReport<'repository> {
    pub results: Vec<CaseResult<'repository>>,
    pub traits_without_cases: Vec<&'repository str>,
}

impl CaseReport<'_> {
    pub fn all_hold(&self) -> bool {
        self.results.iter().all(CaseResult::holds)
    }
}

struct Case<'repository> {
    definition: &'repository TraitDefinition,
    path: String,
    neutral_path: String,
    expected: Answer,
    contents: String,
}

impl Case<'_> {
    fn request(&self) -> JudgeRequest<'_> {
        JudgeRequest {
            state: SnippetState {
                path: &self.neutral_path,
                language: language_name(&self.path),
                signature: None,
                snippet: &self.contents,
            },
            traits: vec![self.definition],
        }
    }
}

pub async fn test_cases<'repository, J: Judge>(
    repository: &'repository ConfiguredRepository,
    slugs: &[String],
    judge: &J,
    cache: &VerdictCache,
) -> Result<CaseReport<'repository>, CaseError> {
    if let Some(slug) = slugs
        .iter()
        .find(|slug| repository.definition(slug).is_none())
    {
        return UnknownTraitSnafu { slug }.fail();
    }
    let chosen = repository
        .definitions()
        .iter()
        .filter(|definition| slugs.is_empty() || slugs.contains(&definition.slug));

    let mut cases = Vec::new();
    let mut traits_without_cases = Vec::new();
    for definition in chosen {
        let found = cases_of(repository.git().root(), definition)?;
        if found.is_empty() {
            traits_without_cases.push(definition.slug.as_str());
        }
        cases.extend(found);
    }

    let requests: Vec<JudgeRequest> = cases.iter().map(Case::request).collect();
    let judged = judge_through_cache(judge, cache, &requests).await?;
    let results = cases
        .into_iter()
        .zip(judged.into_iter().flatten())
        .map(|(case, judged)| CaseResult {
            definition: case.definition,
            path: case.path,
            expected: case.expected,
            verdict: judged.verdict,
            from_cache: judged.from_cache,
        })
        .collect();
    Ok(CaseReport {
        results,
        traits_without_cases,
    })
}

fn cases_of<'repository>(
    repository_root: &Path,
    definition: &'repository TraitDefinition,
) -> Result<Vec<Case<'repository>>, CaseError> {
    let trait_directory = Path::new(TRAITS_DIRECTORY).join(&definition.slug);
    let mut cases = Vec::new();
    for (folder, expected) in CASE_DIRECTORIES {
        let directory = repository_root.join(&trait_directory).join(folder);
        if !directory.is_dir() {
            continue;
        }
        let files = visible_entries(&directory)
            .context(ReadCaseSnafu { path: &directory })?
            .into_iter()
            .filter(|entry| entry.path().is_file());
        for file in files {
            let name = file.file_name().to_string_lossy().into_owned();
            let contents =
                fs::read_to_string(file.path()).context(ReadCaseSnafu { path: file.path() })?;
            cases.push(Case {
                definition,
                path: format!("{}/{folder}/{name}", trait_directory.display()),
                neutral_path: neutral_path(&name),
                expected,
                contents,
            });
        }
    }
    Ok(cases)
}

fn neutral_path(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(
            || CASE_FILE_STEM.to_string(),
            |extension| format!("{CASE_FILE_STEM}.{extension}"),
        )
}

#[cfg(test)]
mod tests;
