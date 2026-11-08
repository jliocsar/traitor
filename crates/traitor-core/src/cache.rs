use std::fs;
use std::io;
use std::path::PathBuf;
use std::process;

use sha2::{Digest, Sha256};

use crate::question::{Question, SnippetState};
use crate::verdict::Verdict;

pub struct VerdictCache {
    directory: Option<PathBuf>,
}

impl VerdictCache {
    pub fn in_user_cache_directory() -> Self {
        Self {
            directory: dirs::cache_dir().map(|cache| cache.join("traitor").join("v1")),
        }
    }

    pub fn at(directory: PathBuf) -> Self {
        Self {
            directory: Some(directory),
        }
    }

    pub fn key(model: &str, question: &Question, state: &SnippetState) -> String {
        let material = serde_json::to_vec(&(model, question, state))
            .expect("a question and a snippet always serialize");
        Sha256::digest(material)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn read(&self, key: &str) -> Option<Verdict> {
        let path = self.directory.as_ref()?.join(format!("{key}.json"));
        let contents = fs::read(path).ok()?;
        serde_json::from_slice(&contents).ok()
    }

    pub fn write(&self, key: &str, verdict: &Verdict) -> io::Result<()> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };
        fs::create_dir_all(directory)?;
        let temporary = directory.join(format!("{key}.json.{}.tmp", process::id()));
        fs::write(&temporary, serde_json::to_vec(verdict)?)?;
        fs::rename(&temporary, directory.join(format!("{key}.json")))
    }
}
