use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use snafu::Snafu;
use tempfile::TempDir;

use crate::judge::{Judge, JudgeRequest};
use crate::trait_definition::TraitDefinition;
use crate::verdict::{Answer, Probabilities, Verdict};

pub const TRAIT: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\nexpect: A computed value flows through combinators, not a reassigned let.\npaths:\n  - \"src/**/*.ts\"\n---\n";

#[derive(Debug, Snafu)]
#[snafu(display("offline"))]
pub struct Offline;

pub struct FakeJudge {
    pub calls: Cell<usize>,
    pub states: RefCell<Vec<Value>>,
    offline: bool,
}

impl FakeJudge {
    pub fn new() -> Self {
        Self {
            calls: Cell::new(0),
            states: RefCell::new(Vec::new()),
            offline: false,
        }
    }

    pub fn offline() -> Self {
        Self {
            offline: true,
            ..Self::new()
        }
    }
}

impl Judge for FakeJudge {
    type Error = Offline;

    fn model(&self) -> &str {
        "fake"
    }

    async fn judge(
        &self,
        request: &JudgeRequest<'_>,
    ) -> Result<BTreeMap<String, Verdict>, Offline> {
        self.calls.set(self.calls.get() + 1);
        self.states
            .borrow_mut()
            .push(serde_json::to_value(request.state).unwrap());
        if self.offline {
            return Err(Offline);
        }
        let fails = request.state.snippet.contains("let ");
        Ok(request
            .traits
            .iter()
            .map(|definition| (definition.slug.clone(), verdict(fails)))
            .collect())
    }
}

pub fn verdict(fails: bool) -> Verdict {
    let fail = if fails { 0.97 } else { 0.01 };
    Verdict {
        answer: if fails { Answer::Fail } else { Answer::Pass },
        confidence: 0.9,
        probabilities: Probabilities {
            pass: 1.0 - fail,
            fail,
            not_applicable: 0.0,
            unsure: 0.0,
        },
    }
}

pub fn railway_definition() -> TraitDefinition {
    TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), TRAIT).unwrap()
}

pub fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=traitor",
            "-c",
            "user.email=traitor@example.com",
        ])
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?}");
}

pub fn repository_with_trait(config: &str) -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    git(root, &["init", "-q", "-b", "main"]);
    fs::create_dir_all(root.join(".agents/traits/railway-not-guards")).unwrap();
    fs::write(
        root.join(".agents/traits/railway-not-guards/TRAIT.md"),
        TRAIT,
    )
    .unwrap();
    fs::write(root.join(".agents/traitor.json"), config).unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "traits"]);
    directory
}
