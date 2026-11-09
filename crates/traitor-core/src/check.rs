use std::collections::BTreeMap;
use std::path::Path;

use snafu::Snafu;

use crate::cache::VerdictCache;
use crate::climb::{Snippet, language_name, snippets_for_changes};
use crate::config::{CONFIG_PATH, Config, ConfigError};
use crate::git::{GitError, Repository};
use crate::judge::{Judge, JudgeRequest, JudgingError, judge_through_cache};
use crate::question::{Question, SnippetState};
use crate::trait_definition::{
    TRAITS_DIRECTORY, TraitDefinition, TraitDefinitionError, load_trait_definitions,
};
use crate::verdict::Verdict;

#[derive(Debug, Snafu)]
pub enum CheckError {
    #[snafu(transparent)]
    Git { source: GitError },
    #[snafu(transparent)]
    Config { source: ConfigError },
    #[snafu(transparent)]
    TraitDefinitions { source: TraitDefinitionError },
    #[snafu(display(
        "{CONFIG_PATH} configures traits.{slug}, but there's no {TRAITS_DIRECTORY}/{slug}"
    ))]
    UnknownTrait { slug: String },
    #[snafu(transparent)]
    Judging { source: JudgingError },
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub path: String,
    pub snippet: Snippet,
    pub trait_slug: String,
    pub trait_title: String,
    pub why: String,
    pub expect: String,
    pub verdict: Verdict,
    pub fails_check: bool,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub snippet_count: usize,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn fails_check(&self) -> bool {
        self.findings.iter().any(|finding| finding.fails_check)
    }
}

struct PendingSnippet<'definitions> {
    path: String,
    snippet: Snippet,
    definitions: Vec<&'definitions TraitDefinition>,
}

pub struct Setup {
    pub repository: Repository,
    pub config: Config,
    pub definitions: Vec<TraitDefinition>,
}

pub fn set_up(directory: &Path) -> Result<Setup, CheckError> {
    let repository = Repository::discover(directory)?;
    let config = Config::load(repository.root())?;
    let definitions = load_trait_definitions(repository.root())?;
    if let Some(slug) = config.configured_traits().find(|slug| {
        !definitions
            .iter()
            .any(|definition| definition.slug == *slug)
    }) {
        return UnknownTraitSnafu { slug }.fail();
    }
    Ok(Setup {
        repository,
        config,
        definitions,
    })
}

pub async fn check<J: Judge>(
    directory: &Path,
    revision: Option<&str>,
    judge: &J,
    cache: &VerdictCache,
) -> Result<Report, CheckError> {
    let Setup {
        repository,
        config,
        definitions,
    } = set_up(directory)?;
    let comparison = repository.resolve_comparison(revision)?;

    let mut pending = Vec::new();
    let mut requests = Vec::new();
    for file in repository.changed_files(&comparison)? {
        let applicable: Vec<&TraitDefinition> = definitions
            .iter()
            .filter(|definition| {
                definition.applies_to(&file.path) && !config.is_exempt(&definition.slug, &file.path)
            })
            .collect();
        if applicable.is_empty() {
            continue;
        }
        let source = repository.read_file(&comparison, &file.path)?;
        let language = language_name(&file.path);
        for snippet in snippets_for_changes(&file.path, &source, &file.changes) {
            requests.push(JudgeRequest {
                state: SnippetState {
                    path: file.path.clone(),
                    language: language.clone(),
                    signature: snippet.signature.clone(),
                    snippet: snippet.text.clone(),
                },
                questions: applicable
                    .iter()
                    .map(|definition| (definition.slug.clone(), Question::about(definition)))
                    .collect::<BTreeMap<_, _>>(),
            });
            pending.push(PendingSnippet {
                path: file.path.clone(),
                snippet,
                definitions: applicable.clone(),
            });
        }
    }

    let answers = judge_through_cache(judge, cache, &requests).await?;
    let findings = pending
        .iter()
        .zip(answers)
        .flat_map(|(item, mut answers)| {
            item.definitions
                .iter()
                .filter_map(|definition| {
                    let judged = answers.remove(&definition.slug)?;
                    Some(Finding {
                        path: item.path.clone(),
                        snippet: item.snippet.clone(),
                        trait_slug: definition.slug.clone(),
                        trait_title: definition.title.clone(),
                        why: definition.why.clone(),
                        expect: definition.expect.clone(),
                        fails_check: config.threshold(&definition.slug).is_some_and(|threshold| {
                            judged.verdict.probabilities.fail >= threshold
                        }),
                        verdict: judged.verdict,
                        from_cache: judged.from_cache,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Ok(Report {
        snippet_count: pending.len(),
        findings,
    })
}

#[cfg(test)]
mod tests;
