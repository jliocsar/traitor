use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use snafu::{ResultExt, Snafu};

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

#[derive(Debug, Snafu)]
pub enum GitError {
    #[snafu(display("couldn't run git: {source}"))]
    Spawn { source: std::io::Error },
    #[snafu(display("`git {arguments}` failed: {stderr}"))]
    Failed { arguments: String, stderr: String },
    #[snafu(display("couldn't read {path}: {source}"))]
    ReadWorkingTree {
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineRange {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

impl LineRange {
    pub(crate) const WHOLE_FILE: Self = Self {
        first: 1,
        last: usize::MAX,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangedFile {
    pub(crate) path: String,
    pub(crate) changes: Vec<LineRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Comparison {
    base: String,
    head: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Repository {
    root: PathBuf,
}

impl Repository {
    pub(crate) fn discover(directory: &Path) -> Result<Self, GitError> {
        let root = run_git(directory, &["rev-parse", "--show-toplevel"])?;
        Ok(Self {
            root: PathBuf::from(root.trim_end()),
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn resolve_comparison(
        &self,
        revision: Option<&str>,
    ) -> Result<Comparison, GitError> {
        let Some(revision) = revision else {
            return Ok(Comparison {
                base: self.default_base(),
                head: None,
            });
        };
        if let Some((from, to)) = revision.split_once("...") {
            let to = or_head(to);
            let base = self.git(&["merge-base", or_head(from), to])?;
            return Ok(Comparison {
                base: base.trim_end().to_string(),
                head: Some(to.to_string()),
            });
        }
        if let Some((from, to)) = revision.split_once("..") {
            return Ok(Comparison {
                base: or_head(from).to_string(),
                head: Some(or_head(to).to_string()),
            });
        }
        Ok(Comparison {
            base: revision.to_string(),
            head: None,
        })
    }

    pub(crate) fn changed_files(
        &self,
        comparison: &Comparison,
    ) -> Result<Vec<ChangedFile>, GitError> {
        let mut arguments = vec![
            "-c",
            "core.quotePath=false",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            "--unified=0",
            "--diff-filter=d",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            &comparison.base,
        ];
        arguments.extend(comparison.head.as_deref());
        arguments.push("--");
        let mut files = parse_diff(&self.git(&arguments)?);
        if comparison.head.is_none() {
            files.extend(self.untracked_files()?);
        }
        files.sort_by(|left, right| left.path.cmp(&right.path));
        Ok(files)
    }

    pub(crate) fn read_file(
        &self,
        comparison: &Comparison,
        path: &str,
    ) -> Result<String, GitError> {
        match &comparison.head {
            Some(head) => self.git(&["show", &format!("{head}:{path}")]),
            None => std::fs::read(self.root.join(path))
                .map(text_from_bytes)
                .context(ReadWorkingTreeSnafu { path }),
        }
    }

    fn default_base(&self) -> String {
        let Ok(head) = self.git(&["rev-parse", "--verify", "--quiet", "HEAD^{commit}"]) else {
            return EMPTY_TREE.to_string();
        };
        ["@{upstream}", "origin/HEAD"]
            .iter()
            .find_map(|reference| self.git(&["merge-base", "HEAD", reference]).ok())
            .unwrap_or(head)
            .trim_end()
            .to_string()
    }

    fn untracked_files(&self) -> Result<Vec<ChangedFile>, GitError> {
        let listing = self.git(&["ls-files", "--others", "--exclude-standard", "-z"])?;
        Ok(listing
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(|path| ChangedFile {
                path: path.to_string(),
                changes: vec![LineRange::WHOLE_FILE],
            })
            .collect())
    }

    fn git(&self, arguments: &[&str]) -> Result<String, GitError> {
        run_git(&self.root, arguments)
    }
}

fn or_head(revision: &str) -> &str {
    if revision.is_empty() {
        "HEAD"
    } else {
        revision
    }
}

fn run_git(directory: &Path, arguments: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .context(SpawnSnafu)?;
    if !output.status.success() {
        return FailedSnafu {
            arguments: arguments.join(" "),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        }
        .fail();
    }
    Ok(text_from_bytes(output.stdout))
}

fn text_from_bytes(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

fn parse_diff(diff: &str) -> Vec<ChangedFile> {
    let mut files: Vec<ChangedFile> = Vec::new();
    let mut in_file_header = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            in_file_header = true;
        } else if in_file_header && let Some(path) = line.strip_prefix("+++ b/") {
            files.push(ChangedFile {
                path: path.to_string(),
                changes: Vec::new(),
            });
        } else if let Some(range) = line.strip_prefix("@@ ").and_then(parse_hunk_header)
            && let Some(file) = files.last_mut()
        {
            in_file_header = false;
            file.changes.push(range);
        }
    }
    files.retain(|file| !file.changes.is_empty());
    files
}

fn parse_hunk_header(header: &str) -> Option<LineRange> {
    let added = header.split(' ').nth(1)?.strip_prefix('+')?;
    let (start, count): (usize, usize) = match added.split_once(',') {
        Some((start, count)) => (start.parse().ok()?, count.parse().ok()?),
        None => (added.parse().ok()?, 1),
    };
    match count {
        0 => Some(LineRange {
            first: start.max(1),
            last: start.max(1),
        }),
        _ => Some(LineRange {
            first: start,
            last: start + count - 1,
        }),
    }
}

#[cfg(test)]
mod tests;
