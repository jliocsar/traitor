use crate::question::{Question, SnippetState};
use crate::store::{JsonStore, hex_sha256};
use crate::verdict::Verdict;

pub(crate) struct VerdictCache {
    store: JsonStore,
}

impl VerdictCache {
    pub(crate) fn new(store: &JsonStore) -> Self {
        Self {
            store: store.clone(),
        }
    }

    pub(crate) fn key(model: &str, question: &Question, state: &SnippetState) -> String {
        let material = serde_json::to_vec(&(model, question, state))
            .expect("a question and a snippet always serialize");
        hex_sha256(&material)
    }

    pub(crate) fn read(&self, key: &str) -> Option<Verdict> {
        self.store.read(key)
    }

    pub(crate) fn write(&self, key: &str, verdict: &Verdict) {
        self.store.write_best_effort(key, verdict);
    }
}
