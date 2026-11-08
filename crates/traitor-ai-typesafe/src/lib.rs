use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};
use traitor_core::judge::{Judge, JudgeRequest};
use traitor_core::question::{Question, SnippetState};
use traitor_core::verdict::{Answer, Probabilities, Verdict};

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MODEL: &str = "jev-latest";
const API_KEY_VARIABLE: &str = "TYPESAFE_API_KEY";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Snafu)]
pub enum TypeSafeError {
    #[snafu(display(
        "{API_KEY_VARIABLE} isn't set, so traitor can't ask TypeSafe's Jev for verdicts"
    ))]
    MissingApiKey,
    #[snafu(display("couldn't reach TypeSafe: {source}"))]
    Request { source: reqwest::Error },
    #[snafu(display("TypeSafe answered {status}: {body}"))]
    Status { status: u16, body: String },
    #[snafu(display("couldn't read TypeSafe's answer: {source}"))]
    Decode { source: reqwest::Error },
    #[snafu(display("TypeSafe answered without a verdict for {slug}"))]
    MissingAnswer { slug: String },
}

pub struct TypeSafeJudge {
    client: reqwest::Client,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct SystemOneRequest<'request> {
    model: &'static str,
    state: &'request SnippetState,
    questions: BTreeMap<&'request str, ChoiceQuestion<'request>>,
}

#[derive(Serialize)]
struct ChoiceQuestion<'request> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    question: &'request Question,
}

#[derive(Deserialize)]
struct SystemOneResponse {
    answers: BTreeMap<String, ChoiceAnswer>,
}

#[derive(Deserialize)]
struct ChoiceAnswer {
    choice: Answer,
    confidence: f64,
    probabilities: Probabilities,
}

impl TypeSafeJudge {
    pub fn from_environment() -> Self {
        Self::new(
            std::env::var(API_KEY_VARIABLE)
                .ok()
                .filter(|key| !key.trim().is_empty()),
        )
    }

    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .unwrap_or_default(),
            api_key,
        }
    }
}

impl Judge for TypeSafeJudge {
    type Error = TypeSafeError;

    fn model(&self) -> &str {
        MODEL
    }

    async fn judge(
        &self,
        request: &JudgeRequest,
    ) -> Result<BTreeMap<String, Verdict>, TypeSafeError> {
        let api_key = self.api_key.as_deref().context(MissingApiKeySnafu)?;
        let response = self
            .client
            .post(ENDPOINT)
            .bearer_auth(api_key)
            .json(&request_body(request))
            .send()
            .await
            .context(RequestSnafu)?;
        let status = response.status();
        if !status.is_success() {
            return StatusSnafu {
                status: status.as_u16(),
                body: response.text().await.unwrap_or_default(),
            }
            .fail();
        }
        let decoded: SystemOneResponse = response.json().await.context(DecodeSnafu)?;
        verdicts(request, decoded)
    }
}

fn request_body(request: &JudgeRequest) -> SystemOneRequest<'_> {
    SystemOneRequest {
        model: MODEL,
        state: &request.state,
        questions: request
            .questions
            .iter()
            .map(|(slug, question)| {
                (
                    slug.as_str(),
                    ChoiceQuestion {
                        kind: "choice",
                        question,
                    },
                )
            })
            .collect(),
    }
}

fn verdicts(
    request: &JudgeRequest,
    mut response: SystemOneResponse,
) -> Result<BTreeMap<String, Verdict>, TypeSafeError> {
    request
        .questions
        .keys()
        .map(|slug| {
            let answer = response
                .answers
                .remove(slug)
                .context(MissingAnswerSnafu { slug })?;
            Ok((
                slug.clone(),
                Verdict {
                    answer: answer.choice,
                    confidence: answer.confidence,
                    probabilities: answer.probabilities,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use traitor_core::trait_definition::TraitDefinition;

    use super::*;

    fn request() -> JudgeRequest {
        let definition = TraitDefinition::parse(
            "railway-not-guards",
            Path::new("TRAIT.md"),
            "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\nexpect: A computed value flows through combinators.\npaths: [\"src/**/*.ts\"]\n---\n## Fail\n```ts\nlet total = 0\n```\n",
        )
        .unwrap();
        JudgeRequest {
            state: SnippetState {
                path: "src/module.ts".to_string(),
                language: "typescript".to_string(),
                signature: None,
                snippet: "const LIMIT = 10".to_string(),
            },
            questions: BTreeMap::from([(definition.slug.clone(), Question::about(&definition))]),
        }
    }

    #[test]
    fn the_request_body_is_one_choice_per_trait() {
        insta::assert_json_snapshot!(request_body(&request()));
    }

    #[test]
    fn reads_a_choice_answer_as_a_verdict() {
        let response: SystemOneResponse = serde_json::from_str(
            r#"{"model":"jev-1.14.0","usage":{},"answers":{"railway-not-guards":{"type":"choice","choice":"n/a","confidence":0.76,"probabilities":{"pass":0.1,"fail":0.01,"n/a":0.89,"unsure":0}}}}"#,
        )
        .unwrap();
        let verdicts = verdicts(&request(), response).unwrap();
        assert_eq!(verdicts["railway-not-guards"].answer, Answer::NotApplicable);
        assert_eq!(
            verdicts["railway-not-guards"].probabilities.not_applicable,
            0.89
        );
    }
}
