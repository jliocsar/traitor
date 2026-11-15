//! Asking a judge for verdicts, through a cache keyed by the model, the question and the snippet.

use std::collections::BTreeMap;

use futures::{StreamExt, TryStreamExt, stream};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::question::{Question, SnippetState};
use crate::store::{CacheWriteError, JsonStore, hex_sha256};
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

    /// The model behind the verdicts. It is part of every cache key, so it has to change
    /// whenever the judge could answer the same question differently.
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
        // ast-grep-ignore: no-box-dyn-error -- a type parameter for J::Error would thread through every caller's error for no caller's benefit
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[snafu(display("couldn't build a cache key: {source}"))]
    CacheKey { source: serde_json::Error },
    #[snafu(display("the judge answered without a verdict for {slug}"))]
    MissingVerdict { slug: String },
}

#[derive(Debug, Clone)]
pub(crate) struct Judged<'repository> {
    pub(crate) definition: &'repository TraitDefinition,
    pub(crate) verdict: Verdict,
    pub(crate) from_cache: bool,
}

/// Each item with its verdicts, and the first verdict that couldn't be cached, if any.
pub(crate) struct Judgements<'repository, T> {
    pub(crate) judged: Vec<(T, Vec<Judged<'repository>>)>,
    pub(crate) cache_write_error: Option<CacheWriteError>,
}

struct CachedQuestion<'repository> {
    definition: &'repository TraitDefinition,
    key: String,
    verdict: Option<Verdict>,
}

pub(crate) async fn judge_through_cache<'repository, T, J: Judge>(
    judge: &J,
    store: &JsonStore,
    items: Vec<T>,
    request: impl Fn(&T) -> JudgeRequest<'_, 'repository>,
) -> Result<Judgements<'repository, T>, JudgingError> {
    let (judged, cache_write_error) = {
        let requests: Vec<JudgeRequest> = items.iter().map(request).collect();

        judge_requests(judge, store, &requests).await?
    };

    Ok(Judgements {
        judged: items.into_iter().zip(judged).collect(),
        cache_write_error,
    })
}

fn cache_key(
    model: &str,
    question: &Question,
    state: &SnippetState,
) -> Result<String, JudgingError> {
    let material = serde_json::to_vec(&(model, question, state)).context(CacheKeySnafu)?;

    Ok(hex_sha256(&material))
}

async fn judge_requests<'repository, J: Judge>(
    judge: &J,
    store: &JsonStore,
    requests: &[JudgeRequest<'_, 'repository>],
) -> Result<(Vec<Vec<Judged<'repository>>>, Option<CacheWriteError>), JudgingError> {
    let cached: Vec<Vec<CachedQuestion>> = requests
        .iter()
        .map(|request| {
            request
                .traits
                .iter()
                .map(|definition| {
                    let key =
                        cache_key(judge.model(), &Question::about(definition), &request.state)?;

                    Ok(CachedQuestion {
                        definition,
                        verdict: store.read(&key),
                        key,
                    })
                })
                .collect::<Result<_, JudgingError>>()
        })
        .collect::<Result<_, _>>()?;
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
    let asked = &cached;
    let answered: Vec<(usize, BTreeMap<String, Verdict>, Option<CacheWriteError>)> =
        stream::iter(uncached)
            .map(|(index, request)| async move {
                let verdicts = judge.judge(&request).await?;
                let cache_write_error = asked[index]
                    .iter()
                    .filter_map(|question| {
                        Some((question, verdicts.get(&question.definition.slug)?))
                    })
                    .map(|(question, verdict)| store.write(&question.key, verdict))
                    .fold(None, |first, written| first.or(written.err()));

                Ok::<_, J::Error>((index, verdicts, cache_write_error))
            })
            .buffer_unordered(CONCURRENT_REQUESTS)
            .try_collect()
            .await
            .map_err(|error| JudgingError::Judge {
                source: Box::new(error),
            })?;
    let (mut answers, cache_write_error) = answered.into_iter().fold(
        (BTreeMap::new(), None),
        |(mut answers, first), (index, verdicts, error)| {
            answers.insert(index, verdicts);

            (answers, first.or(error))
        },
    );
    let judged = cached
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

                    Ok(Judged {
                        definition: question.definition,
                        verdict,
                        from_cache: false,
                    })
                })
                .collect()
        })
        .collect::<Result<_, _>>()?;

    Ok((judged, cache_write_error))
}
