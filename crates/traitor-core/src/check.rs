use snafu::Snafu;

use crate::cache::VerdictCache;
use crate::climb::{Snippet, language_name, snippets_for_changes};
use crate::configured_repository::ConfiguredRepository;
use crate::git::GitError;
use crate::judge::{Judge, JudgeRequest, JudgingError, judge_through_cache};
use crate::question::SnippetState;
use crate::store::JsonStore;
use crate::trait_definition::TraitDefinition;
use crate::verdict::Verdict;

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
    pub fails_check: bool,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Report<'repository> {
    pub snippet_count: usize,
    pub findings: Vec<Finding<'repository>>,
}

impl Report<'_> {
    pub fn fails_check(&self) -> bool {
        self.findings.iter().any(|finding| finding.fails_check)
    }
}

struct ChangedSnippet<'repository> {
    path: String,
    snippet: Snippet,
    traits: Vec<&'repository TraitDefinition>,
}

impl ChangedSnippet<'_> {
    fn request(&self) -> JudgeRequest<'_> {
        JudgeRequest {
            state: SnippetState {
                path: &self.path,
                language: language_name(&self.path),
                signature: self.snippet.signature.as_deref(),
                snippet: &self.snippet.text,
            },
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

    let requests: Vec<JudgeRequest> = changed.iter().map(ChangedSnippet::request).collect();
    let judged = judge_through_cache(judge, &VerdictCache::new(store), &requests).await?;
    let findings = changed
        .iter()
        .zip(judged)
        .flat_map(|(changed, judged)| {
            changed
                .traits
                .iter()
                .zip(judged)
                .map(|(definition, judged)| Finding {
                    path: changed.path.clone(),
                    snippet: changed.snippet.clone(),
                    definition,
                    fails_check: repository.fails_check(definition, &judged.verdict),
                    verdict: judged.verdict,
                    from_cache: judged.from_cache,
                })
        })
        .collect();
    Ok(Report {
        snippet_count: changed.len(),
        findings,
    })
}

#[cfg(test)]
mod tests;
