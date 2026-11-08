use std::collections::BTreeMap;

use crate::question::{Question, SnippetState};
use crate::verdict::Verdict;

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
