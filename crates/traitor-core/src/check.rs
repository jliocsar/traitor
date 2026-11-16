//! Judging every changed snippet, or every snippet of the files asked for, against the traits whose paths match it.

use std::io;
use std::path::{Path, PathBuf};

use snafu::{ResultExt, Snafu};

use crate::climb::{Snippet, snippets_for_changes};
use crate::configured_repository::ConfiguredRepository;
use crate::git::{Change, ChangedFile, ChangedFiles, GitError};
use crate::judge::{Judge, JudgeRequest, JudgingError, judge_through_cache};
use crate::question::SnippetState;
use crate::store::{CacheWriteError, JsonStore};
use crate::trait_definition::TraitDefinition;
use crate::verdict::{Answer, Verdict};

#[derive(Debug, Snafu)]
pub enum CheckError {
    #[snafu(transparent)]
    Git { source: GitError },
    #[snafu(transparent)]
    Judging { source: JudgingError },
    #[snafu(display("couldn't find {}: {source}", path.display()))]
    FindFile { path: PathBuf, source: io::Error },
    #[snafu(display(
        "{} isn't in the repository at {}, so no trait can apply to it",
        path.display(),
        root.display()
    ))]
    OutsideRepository { path: PathBuf, root: PathBuf },
}

/// What `check` judges.
#[derive(Debug, Clone, Copy)]
pub enum Scope<'request> {
    /// What changed, as `git diff` reads the revision: none for everything unpushed.
    Changes(Option<&'request str>),
    /// These files, whole, as they are on disk. A relative path is read from the current
    /// directory.
    Files(&'request [PathBuf]),
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
    #[must_use]
    pub fn fails_check(&self) -> bool {
        self.threshold
            .is_some_and(|threshold| self.verdict.probabilities.fail >= threshold)
    }

    #[must_use]
    pub fn location(&self) -> String {
        format!(
            "{}:{}-{}",
            self.path, self.snippet.first_line, self.snippet.last_line
        )
    }
}

#[derive(Debug, Default)]
pub struct Report<'repository> {
    pub snippet_count: usize,
    pub findings: Vec<Finding<'repository>>,
    /// Changed paths that aren't valid UTF-8, shown as near as they can be. They weren't judged.
    pub non_utf8_paths: Vec<String>,
    /// The first verdict that couldn't be cached. The verdicts are the same; a later run asks again.
    pub cache_write_error: Option<CacheWriteError>,
}

impl Report<'_> {
    #[must_use]
    pub fn fails_check(&self) -> bool {
        self.findings.iter().any(Finding::fails_check)
    }

    /// Whether there are fails, and none of their traits has a threshold to fail the check with.
    #[must_use]
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

/// Judges every snippet in `scope` against the traits whose paths match it.
///
/// # Errors
///
/// When git fails, a file asked for can't be found or sits outside the repository, or the
/// judge can't give a verdict.
pub async fn check<'repository, J: Judge>(
    repository: &'repository ConfiguredRepository,
    scope: Scope<'_>,
    judge: &J,
    store: &JsonStore,
) -> Result<Report<'repository>, CheckError> {
    let git = repository.git();

    let (changed_files, comparison) = match scope {
        Scope::Changes(revision) => {
            let comparison = git.resolve_comparison(revision)?;

            (git.changed_files(&comparison)?, Some(comparison))
        }
        Scope::Files(paths) => (whole_files(git.root(), paths)?, None),
    };

    let mut changed = Vec::new();

    for file in changed_files.files {
        let traits = repository.traits_for(&file.path);

        if traits.is_empty() {
            continue;
        }

        let read = match &comparison {
            Some(comparison) => git.read_file(comparison, &file.path)?,
            None => git.read_working_file(&file.path)?,
        };

        let Some(source) = read else {
            continue;
        };

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

    let judgements = judge_through_cache(judge, store, changed, ChangedSnippet::request).await?;
    let snippet_count = judgements.judged.len();
    let findings = judgements
        .judged
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
        non_utf8_paths: changed_files.non_utf8_paths,
        cache_write_error: judgements.cache_write_error,
    })
}

/// `paths` as files to judge whole, each by its path in the repository at `root`, sorted and
/// each once.
fn whole_files(root: &Path, paths: &[PathBuf]) -> Result<ChangedFiles, CheckError> {
    let root = root.canonicalize().context(FindFileSnafu { path: root })?;
    let mut files = ChangedFiles::default();

    for path in paths {
        let found = path.canonicalize().context(FindFileSnafu { path })?;

        let Ok(relative) = found.strip_prefix(&root) else {
            return OutsideRepositorySnafu { path, root }.fail();
        };

        match repository_path(relative) {
            Some(repository_path) => files.files.push(ChangedFile {
                path: repository_path,
                changes: vec![Change::WHOLE_FILE],
            }),
            None => files
                .non_utf8_paths
                .push(relative.to_string_lossy().into_owned()),
        }
    }

    files
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    files.files.dedup_by(|left, right| left.path == right.path);

    Ok(files)
}

/// `relative` the way git and trait globs write a path: UTF-8, with `/` between its parts.
fn repository_path(relative: &Path) -> Option<String> {
    relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect::<Option<Vec<_>>>()
        .map(|parts| parts.join("/"))
}

#[cfg(test)]
mod tests;
