use snafu::Snafu;

use crate::climb::{Snippet, snippets_for_changes};
use crate::configured_repository::ConfiguredRepository;
use crate::git::GitError;
use crate::judge::{Judge, JudgeRequest, JudgingError, judge_through_cache};
use crate::question::SnippetState;
use crate::store::JsonStore;
use crate::trait_definition::TraitDefinition;
use crate::verdict::{Answer, Verdict};

#[derive(Debug, Snafu)]
pub enum CheckError {
    #[snafu(transparent)]
    Git { source: GitError },
    #[snafu(transparent)]
    Judging { source: JudgingError },
}

#[derive(Debug, Clone)]
pub struct Finding<'repository> {
    pub path: String,
    pub snippet: Snippet,
    pub definition: &'repository TraitDefinition,
    pub verdict: Verdict,
    pub threshold: Option<f64>,
    pub from_cache: bool,
}

impl Finding<'_> {
    pub fn fails_check(&self) -> bool {
        self.threshold
            .is_some_and(|threshold| self.verdict.probabilities.fail >= threshold)
    }

    pub fn location(&self) -> String {
        format!(
            "{}:{}-{}",
            self.path, self.snippet.first_line, self.snippet.last_line
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Report<'repository> {
    pub snippet_count: usize,
    pub findings: Vec<Finding<'repository>>,
}

impl Report<'_> {
    pub fn fails_check(&self) -> bool {
        self.findings.iter().any(Finding::fails_check)
    }

    /// Whether there are fails, and none of their traits has a threshold to fail the check with.
    pub fn fails_are_only_reported(&self) -> bool {
        let mut fails = self
            .findings
            .iter()
            .filter(|finding| finding.verdict.answer == Answer::Fail)
            .peekable();
        fails.peek().is_some() && fails.all(|finding| finding.threshold.is_none())
    }
}

struct ChangedSnippet<'repository> {
    path: String,
    snippet: Snippet,
    traits: Vec<&'repository TraitDefinition>,
}

impl<'repository> ChangedSnippet<'repository> {
    fn request(&self) -> JudgeRequest<'_, 'repository> {
        JudgeRequest {
            state: SnippetState::new(
                &self.path,
                self.snippet.signature.as_deref(),
                &self.snippet.text,
            ),
            traits: self.traits.clone(),
        }
    }
}

pub async fn check<'repository, J: Judge>(
    repository: &'repository ConfiguredRepository,
    revision: Option<&str>,
    judge: &J,
    store: &JsonStore,
) -> Result<Report<'repository>, CheckError> {
    let git = repository.git();
    let comparison = git.resolve_comparison(revision)?;
    let mut changed = Vec::new();
    for file in git.changed_files(&comparison)? {
        let traits = repository.traits_for(&file.path);
        if traits.is_empty() {
            continue;
        }
        let source = git.read_file(&comparison, &file.path)?;
        changed.extend(
            snippets_for_changes(&file.path, &source, &file.changes)
                .into_iter()
                .map(|snippet| ChangedSnippet {
                    path: file.path.clone(),
                    snippet,
                    traits: traits.clone(),
                }),
        );
    }

    let judged = judge_through_cache(judge, store, changed, ChangedSnippet::request).await?;
    let snippet_count = judged.len();
    let findings = judged
        .into_iter()
        .flat_map(|(changed, judged)| {
            judged.into_iter().map(move |judged| Finding {
                path: changed.path.clone(),
                snippet: changed.snippet.clone(),
                definition: judged.definition,
                threshold: repository.threshold(judged.definition),
                verdict: judged.verdict,
                from_cache: judged.from_cache,
            })
        })
        .collect();
    Ok(Report {
        snippet_count,
        findings,
    })
}

#[cfg(test)]
mod tests;
