use std::collections::BTreeMap;

use futures::{StreamExt, TryStreamExt, stream};
use snafu::{OptionExt, Snafu};

use crate::cache::VerdictCache;
use crate::question::{Question, SnippetState};
use crate::trait_definition::TraitDefinition;
use crate::verdict::Verdict;

const CONCURRENT_REQUESTS: usize = 4;

#[derive(Debug, Clone)]
pub struct JudgeRequest<'a> {
    pub state: SnippetState<'a>,
    pub traits: Vec<&'a TraitDefinition>,
}

impl<'a> JudgeRequest<'a> {
    pub fn questions(&self) -> impl Iterator<Item = (&'a str, Question<'a>)> {
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
        request: &JudgeRequest<'_>,
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
pub struct Judged {
    pub verdict: Verdict,
    pub from_cache: bool,
}

struct CachedQuestion {
    key: String,
    verdict: Option<Verdict>,
}

pub async fn judge_through_cache<J: Judge>(
    judge: &J,
    cache: &VerdictCache,
    requests: &[JudgeRequest<'_>],
) -> Result<Vec<Vec<Judged>>, JudgingError> {
    let cached: Vec<Vec<CachedQuestion>> = requests
        .iter()
        .map(|request| {
            request
                .questions()
                .map(|(_, question)| {
                    let key = VerdictCache::key(judge.model(), &question, &request.state);
                    let verdict = cache.read(&key);
                    CachedQuestion { key, verdict }
                })
                .collect()
        })
        .collect();
    let uncached: Vec<(usize, JudgeRequest)> = requests
        .iter()
        .zip(&cached)
        .enumerate()
        .filter_map(|(index, (request, cached))| {
            let traits: Vec<&TraitDefinition> = request
                .traits
                .iter()
                .zip(cached)
                .filter(|(_, question)| question.verdict.is_none())
                .map(|(definition, _)| *definition)
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
    requests
        .iter()
        .zip(cached)
        .enumerate()
        .map(|(index, (request, questions))| {
            request
                .traits
                .iter()
                .zip(questions)
                .map(|(definition, question)| {
                    if let Some(verdict) = question.verdict {
                        return Ok(Judged {
                            verdict,
                            from_cache: true,
                        });
                    }
                    let verdict = answers
                        .get_mut(&index)
                        .and_then(|verdicts| verdicts.remove(&definition.slug))
                        .context(MissingVerdictSnafu {
                            slug: &definition.slug,
                        })?;
                    cache.write(&question.key, &verdict);
                    Ok(Judged {
                        verdict,
                        from_cache: false,
                    })
                })
                .collect()
        })
        .collect()
}
