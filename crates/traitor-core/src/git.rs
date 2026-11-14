//! What changed, read from git: the comparison to judge, the changed files and their contents.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use snafu::{ResultExt, Snafu};

const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const BINARY_SNIFF_BYTES: usize = 8000;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    Added(LineRange),
    Deleted { after: usize },
}

impl Change {
    pub(crate) const WHOLE_FILE: Self = Self::Added(LineRange {
        first: 1,
        last: usize::MAX,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangedFile {
    pub(crate) path: String,
    pub(crate) changes: Vec<Change>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ChangedFiles {
    pub(crate) files: Vec<ChangedFile>,
    /// Changed paths that aren't valid UTF-8, shown as near as they can be. They aren't judged.
    pub(crate) non_utf8_paths: Vec<String>,
}

impl ChangedFiles {
    fn extend(&mut self, other: Self) {
        self.files.extend(other.files);
        self.non_utf8_paths.extend(other.non_utf8_paths);
    }
}

/// A path that isn't valid UTF-8, shown as near as it can be.
#[derive(Debug)]
struct NonUtf8Path {
    shown: String,
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
            let base = self.git(&["merge-base", "--end-of-options", or_head(from), to])?;

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

    pub(crate) fn changed_files(&self, comparison: &Comparison) -> Result<ChangedFiles, GitError> {
        // Quoting keeps every header ASCII, so a path that isn't UTF-8 survives as octal escapes
        // instead of being mangled along with the rest of the diff's text.
        let mut arguments = vec![
            "-c",
            "core.quotePath=true",
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--no-renames",
            "--unified=0",
            "--diff-filter=d",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--end-of-options",
            &comparison.base,
        ];
        arguments.extend(comparison.head.as_deref());
        arguments.push("--");
        let mut changed = parse_diff(&self.git(&arguments)?);

        if comparison.head.is_none() {
            changed.extend(self.untracked_files()?);
        }

        changed
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        Ok(changed)
    }

    pub(crate) fn read_file(
        &self,
        comparison: &Comparison,
        path: &str,
    ) -> Result<Option<String>, GitError> {
        let text = match &comparison.head {
            Some(head) => self.git(&["show", "--end-of-options", &format!("{head}:{path}")])?,
            None => std::fs::read(self.root.join(path))
                .map(text_from_bytes)
                .context(ReadWorkingTreeSnafu { path })?,
        };

        Ok((!looks_binary(&text)).then_some(text))
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

    fn untracked_files(&self) -> Result<ChangedFiles, GitError> {
        let listing = run_git_bytes(
            &self.root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?;
        let (paths, non_utf8_paths): (Vec<_>, Vec<_>) = listing
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| utf8_path(path.to_vec()))
            .partition(Result::is_ok);
        Ok(ChangedFiles {
            files: paths
                .into_iter()
                .flatten()
                .map(|path| ChangedFile {
                    path,
                    changes: vec![Change::WHOLE_FILE],
                })
                .collect(),
            non_utf8_paths: non_utf8_paths
                .into_iter()
                .filter_map(Result::err)
                .map(|path| path.shown)
                .collect(),
        })
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
    run_git_bytes(directory, arguments).map(text_from_bytes)
}

fn run_git_bytes(directory: &Path, arguments: &[&str]) -> Result<Vec<u8>, GitError> {
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

    Ok(output.stdout)
}

/// The path, or when it isn't valid UTF-8, the nearest it can be shown as.
fn utf8_path(bytes: Vec<u8>) -> Result<String, NonUtf8Path> {
    String::from_utf8(bytes).map_err(|error| NonUtf8Path {
        shown: String::from_utf8_lossy(error.as_bytes()).into_owned(),
    })
}

fn text_from_bytes(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

fn looks_binary(text: &str) -> bool {
    text.as_bytes()
        .iter()
        .take(BINARY_SNIFF_BYTES)
        .any(|byte| *byte == 0)
}

fn parse_diff(diff: &str) -> ChangedFiles {
    let mut changed = ChangedFiles::default();
    let mut current: Option<ChangedFile> = None;
    let mut in_file_header = false;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            changed.files.extend(current.take());
            in_file_header = true;
        } else if in_file_header && let Some(path) = line.strip_prefix("+++ ") {
            current = match new_path(path) {
                Some(Ok(path)) => Some(ChangedFile {
                    path,
                    changes: Vec::new(),
                }),
                Some(Err(NonUtf8Path { shown })) => {
                    changed.non_utf8_paths.push(shown);
                    None
                }
                None => None,
            };
        } else if let Some(header) = line.strip_prefix("@@ ") {
            in_file_header = false;

            if let Some(file) = current.as_mut()
                && let Some(range) = parse_hunk_header(header)
            {
                file.changes.push(range);
            }
        }
    }

    changed.files.extend(current);
    changed.files.retain(|file| !file.changes.is_empty());
    changed
}

/// The path in a `+++` header: none when the header can't be read, and an error with the
/// nearest it can be shown as when the path isn't valid UTF-8.
fn new_path(header_path: &str) -> Option<Result<String, NonUtf8Path>> {
    let header_path = header_path.strip_suffix('\t').unwrap_or(header_path);

    let bytes = if header_path.starts_with('"') {
        unquote(header_path)?
    } else {
        header_path.as_bytes().to_vec()
    };

    let path = bytes.strip_prefix(b"b/")?;
    Some(utf8_path(path.to_vec()))
}

fn unquote(quoted: &str) -> Option<Vec<u8>> {
    let mut escaped = quoted.strip_prefix('"')?.strip_suffix('"')?.bytes();
    let mut bytes = Vec::new();

    while let Some(byte) = escaped.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }

        bytes.push(match escaped.next()? {
            b'a' => 0x07,
            b'b' => 0x08,
            b't' => b'\t',
            b'n' => b'\n',
            b'v' => 0x0b,
            b'f' => 0x0c,
            b'r' => b'\r',
            high @ b'0'..=b'3' => {
                let middle = escaped.next().filter(|byte| is_octal_digit(*byte))?;
                let low = escaped.next().filter(|byte| is_octal_digit(*byte))?;
                (high - b'0') * 64 + (middle - b'0') * 8 + (low - b'0')
            }
            other => other,
        });
    }

    Some(bytes)
}

fn is_octal_digit(byte: u8) -> bool {
    matches!(byte, b'0'..=b'7')
}

fn parse_hunk_header(header: &str) -> Option<Change> {
    let added = header.split(' ').nth(1)?.strip_prefix('+')?;

    let (start, count): (usize, usize) = match added.split_once(',') {
        Some((start, count)) => (start.parse().ok()?, count.parse().ok()?),
        None => (added.parse().ok()?, 1),
    };

    Some(match count {
        0 => Change::Deleted { after: start },
        _ => Change::Added(LineRange {
            first: start,
            last: start + count - 1,
        }),
    })
}

#[cfg(test)]
mod tests;
