use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt, Snafu};
use traitor_core::judge::{Judge, JudgeRequest};
use traitor_core::question::{Question, SnippetState};
use traitor_core::verdict::{Answer, Probabilities, Verdict};

pub const API_HOST: &str = "api.typesafe.ai";
const SYSTEM_ONE_PATH: &str = "/v1/systemone";
const MODEL: &str = "jev-1.14.0";
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
    #[snafu(display("couldn't set up an HTTP client for TypeSafe: {source}"))]
    BuildClient { source: reqwest::Error },
}

pub struct TypeSafeJudge {
    client: reqwest::Client,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct SystemOneRequest<'request> {
    model: &'static str,
    state: &'request SnippetState<'request>,
    questions: BTreeMap<&'request str, ChoiceQuestion<'request>>,
}

#[derive(Serialize)]
struct ChoiceQuestion<'request> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    question: Question<'request>,
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
    pub fn from_environment() -> Result<Self, TypeSafeError> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .context(BuildClientSnafu)?,
            api_key: std::env::var(API_KEY_VARIABLE)
                .ok()
                .filter(|key| !key.trim().is_empty()),
        })
    }
}

impl Judge for TypeSafeJudge {
    type Error = TypeSafeError;

    fn model(&self) -> &str {
        MODEL
    }

    async fn judge(
        &self,
        request: &JudgeRequest<'_, '_>,
    ) -> Result<BTreeMap<String, Verdict>, TypeSafeError> {
        let api_key = self.api_key.as_deref().context(MissingApiKeySnafu)?;
        let response = self
            .client
            .post(format!("https://{API_HOST}{SYSTEM_ONE_PATH}"))
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
        Ok(verdicts(decoded))
    }
}

fn request_body<'request>(
    request: &'request JudgeRequest<'request, 'request>,
) -> SystemOneRequest<'request> {
    SystemOneRequest {
        model: MODEL,
        state: &request.state,
        questions: request
            .questions()
            .map(|(slug, question)| {
                (
                    slug,
                    ChoiceQuestion {
                        kind: "choice",
                        question,
                    },
                )
            })
            .collect(),
    }
}

fn verdicts(response: SystemOneResponse) -> BTreeMap<String, Verdict> {
    response
        .answers
        .into_iter()
        .map(|(slug, answer)| {
            (
                slug,
                Verdict {
                    answer: answer.choice,
                    confidence: answer.confidence,
                    probabilities: answer.probabilities,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests;
