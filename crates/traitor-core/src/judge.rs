use std::collections::BTreeMap;

use futures::{StreamExt, TryStreamExt, stream};
use snafu::{OptionExt, Snafu};

use crate::cache::VerdictCache;
use crate::question::{Question, SnippetState};
use crate::trait_definition::TraitDefinition;
use crate::verdict::Verdict;

const CONCURRENT_REQUESTS: usize = 4;

#[derive(Debug, Clone)]
pub struct JudgeRequest<'state, 'repository> {
    pub state: SnippetState<'state>,
    pub traits: Vec<&'repository TraitDefinition>,
}

impl<'repository> JudgeRequest<'_, 'repository> {
    pub fn questions(&self) -> impl Iterator<Item = (&'repository str, Question<'repository>)> {
        self.traits
            .iter()
            .copied()
            .map(|definition| (definition.slug.as_str(), Question::about(definition)))
    }
}

pub trait Judge {
    type Error: std::error::Error + Send + Sync + 'static;

    fn model(&self) -> &str;

    fn judge(
        &self,
        request: &JudgeRequest<'_, '_>,
    ) -> impl Future<Output = Result<BTreeMap<String, Verdict>, Self::Error>>;
}

#[derive(Debug, Snafu)]
pub enum JudgingError {
    #[snafu(display("{source}"))]
    Judge {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[snafu(display("the judge answered without a verdict for {slug}"))]
    MissingVerdict { slug: String },
}

#[derive(Debug, Clone)]
pub(crate) struct Judged<'repository> {
    pub(crate) definition: &'repository TraitDefinition,
    pub(crate) verdict: Verdict,
    pub(crate) from_cache: bool,
}

struct CachedQuestion<'repository> {
    definition: &'repository TraitDefinition,
    key: String,
    verdict: Option<Verdict>,
}

pub(crate) async fn judge_through_cache<'repository, J: Judge>(
    judge: &J,
    cache: &VerdictCache<'_>,
    requests: &[JudgeRequest<'_, 'repository>],
) -> Result<Vec<Vec<Judged<'repository>>>, JudgingError> {
    let cached: Vec<Vec<CachedQuestion>> = requests
        .iter()
        .map(|request| {
            request
                .traits
                .iter()
                .map(|definition| {
                    let key = VerdictCache::key(
                        judge.model(),
                        &Question::about(definition),
                        &request.state,
                    );
                    CachedQuestion {
                        definition,
                        verdict: cache.read(&key),
                        key,
                    }
                })
                .collect()
        })
        .collect();
    let uncached: Vec<(usize, JudgeRequest)> = requests
        .iter()
        .zip(&cached)
        .enumerate()
        .filter_map(|(index, (request, questions))| {
            let traits: Vec<&TraitDefinition> = questions
                .iter()
                .filter(|question| question.verdict.is_none())
                .map(|question| question.definition)
                .collect();
            (!traits.is_empty()).then_some((
                index,
                JudgeRequest {
                    state: request.state,
                    traits,
                },
            ))
        })
        .collect();
    let mut answers: BTreeMap<usize, BTreeMap<String, Verdict>> = stream::iter(uncached)
        .map(|(index, request)| async move {
            judge
                .judge(&request)
                .await
                .map(|verdicts| (index, verdicts))
        })
        .buffer_unordered(CONCURRENT_REQUESTS)
        .try_collect()
        .await
        .map_err(|error| JudgingError::Judge {
            source: Box::new(error),
        })?;
    cached
        .into_iter()
        .enumerate()
        .map(|(index, questions)| {
            questions
                .into_iter()
                .map(|question| {
                    if let Some(verdict) = question.verdict {
                        return Ok(Judged {
                            definition: question.definition,
                            verdict,
                            from_cache: true,
                        });
                    }
                    let verdict = answers
                        .get_mut(&index)
                        .and_then(|verdicts| verdicts.remove(&question.definition.slug))
                        .context(MissingVerdictSnafu {
                            slug: &question.definition.slug,
                        })?;
                    cache.write(&question.key, &verdict);
                    Ok(Judged {
                        definition: question.definition,
                        verdict,
                        from_cache: false,
                    })
                })
                .collect()
        })
        .collect()
}
