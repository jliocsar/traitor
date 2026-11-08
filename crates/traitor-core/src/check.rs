use std::collections::BTreeMap;
use std::path::Path;

use futures::{StreamExt, TryStreamExt, stream};
use snafu::{OptionExt, Snafu};

use crate::cache::VerdictCache;
use crate::climb::{Snippet, language_name, snippets_for_changes};
use crate::config::{CONFIG_PATH, Config, ConfigError};
use crate::git::{GitError, Repository};
use crate::judge::{Judge, JudgeRequest};
use crate::question::{Question, SnippetState};
use crate::trait_definition::{
    TRAITS_DIRECTORY, TraitDefinition, TraitDefinitionError, load_trait_definitions,
};
use crate::verdict::Verdict;

const CONCURRENT_REQUESTS: usize = 4;

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
    #[snafu(display("{source}"))]
    Judge {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[snafu(display("the judge answered without a verdict for {slug}"))]
    MissingVerdict { slug: String },
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub path: String,
    pub snippet: Snippet,
    pub trait_slug: String,
    pub trait_title: String,
    pub why: String,
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
    state: SnippetState,
    questions: Vec<PendingQuestion<'definitions>>,
}

struct PendingQuestion<'definitions> {
    definition: &'definitions TraitDefinition,
    question: Question,
    cache_key: String,
    cached: Option<Verdict>,
}

pub async fn check<J: Judge>(
    directory: &Path,
    revision: Option<&str>,
    judge: &J,
    cache: &VerdictCache,
) -> Result<Report, CheckError> {
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
    let comparison = repository.resolve_comparison(revision)?;

    let mut pending = Vec::new();
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
            let state = SnippetState {
                path: file.path.clone(),
                language: language.clone(),
                signature: snippet.signature.clone(),
                snippet: snippet.text.clone(),
            };
            let questions = applicable
                .iter()
                .map(|definition| {
                    let question = Question::about(definition);
                    let cache_key = VerdictCache::key(judge.model(), &question, &state);
                    let cached = cache.read(&cache_key);
                    PendingQuestion {
                        definition,
                        question,
                        cache_key,
                        cached,
                    }
                })
                .collect();
            pending.push(PendingSnippet {
                path: file.path.clone(),
                snippet,
                state,
                questions,
            });
        }
    }

    let requests: Vec<(usize, JudgeRequest)> = pending
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let questions: BTreeMap<String, Question> = item
                .questions
                .iter()
                .filter(|question| question.cached.is_none())
                .map(|question| (question.definition.slug.clone(), question.question.clone()))
                .collect();
            (!questions.is_empty()).then(|| {
                (
                    index,
                    JudgeRequest {
                        state: item.state.clone(),
                        questions,
                    },
                )
            })
        })
        .collect();
    let answers: BTreeMap<usize, BTreeMap<String, Verdict>> = stream::iter(requests)
        .map(|(index, request)| async move {
            judge
                .judge(&request)
                .await
                .map(|verdicts| (index, verdicts))
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .try_collect()
        .await
        .map_err(|error| CheckError::Judge {
            source: Box::new(error),
        })?;

    let mut findings = Vec::new();
    for (index, item) in pending.iter().enumerate() {
        for question in &item.questions {
            let slug = &question.definition.slug;
            let (verdict, from_cache) = match &question.cached {
                Some(verdict) => (verdict.clone(), true),
                None => {
                    let verdict = answers
                        .get(&index)
                        .and_then(|verdicts| verdicts.get(slug))
                        .cloned()
                        .context(MissingVerdictSnafu { slug })?;
                    let _ = cache.write(&question.cache_key, &verdict);
                    (verdict, false)
                }
            };
            findings.push(Finding {
                path: item.path.clone(),
                snippet: item.snippet.clone(),
                trait_slug: slug.clone(),
                trait_title: question.definition.title.clone(),
                why: question.definition.why.clone(),
                fails_check: config
                    .threshold(slug)
                    .is_some_and(|threshold| verdict.probabilities.fail >= threshold),
                verdict,
                from_cache,
            });
        }
    }
    Ok(Report {
        snippet_count: pending.len(),
        findings,
    })
}

#[cfg(test)]
mod tests;
