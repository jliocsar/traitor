use std::collections::BTreeMap;

use futures::{StreamExt, TryStreamExt, stream};
use snafu::{OptionExt, Snafu};

use crate::cache::VerdictCache;
use crate::question::{Question, SnippetState};
use crate::verdict::Verdict;

const CONCURRENT_REQUESTS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgeRequest {
    pub state: SnippetState,
    pub questions: BTreeMap<String, Question>,
}

pub trait Judge {
    type Error: std::error::Error + Send + Sync + 'static;

    fn model(&self) -> &str;

    fn judge(
        &self,
        request: &JudgeRequest,
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
    requests: &[JudgeRequest],
) -> Result<Vec<BTreeMap<String, Judged>>, JudgingError> {
    let cached: Vec<BTreeMap<&String, CachedQuestion>> = requests
        .iter()
        .map(|request| {
            request
                .questions
                .iter()
                .map(|(slug, question)| {
                    let key = VerdictCache::key(judge.model(), question, &request.state);
                    let verdict = cache.read(&key);
                    (slug, CachedQuestion { key, verdict })
                })
                .collect()
        })
        .collect();
    let uncached: Vec<(usize, JudgeRequest)> = requests
        .iter()
        .zip(&cached)
        .enumerate()
        .filter_map(|(index, (request, cached))| {
            let questions: BTreeMap<String, Question> = request
                .questions
                .iter()
                .filter(|(slug, _)| cached[slug].verdict.is_none())
                .map(|(slug, question)| (slug.clone(), question.clone()))
                .collect();
            (!questions.is_empty()).then(|| {
                (
                    index,
                    JudgeRequest {
                        state: request.state.clone(),
                        questions,
                    },
                )
            })
        })
        .collect();
    let answers: BTreeMap<usize, BTreeMap<String, Verdict>> = stream::iter(uncached)
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
                .map(|(slug, question)| {
                    let judged = match question.verdict {
                        Some(verdict) => Judged {
                            verdict,
                            from_cache: true,
                        },
                        None => {
                            let verdict = answers
                                .get(&index)
                                .and_then(|verdicts| verdicts.get(slug))
                                .cloned()
                                .context(MissingVerdictSnafu { slug })?;
                            let _ = cache.write(&question.key, &verdict);
                            Judged {
                                verdict,
                                from_cache: false,
                            }
                        }
                    };
                    Ok((slug.clone(), judged))
                })
                .collect()
        })
        .collect()
}
