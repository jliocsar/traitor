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
    ) -> Result<Option<String>, GitError> {
        let text = match &comparison.head {
            Some(head) => self.git(&["show", &format!("{head}:{path}")])?,
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

fn looks_binary(text: &str) -> bool {
    text.as_bytes()
        .iter()
        .take(BINARY_SNIFF_BYTES)
        .any(|byte| *byte == 0)
}

fn parse_diff(diff: &str) -> Vec<ChangedFile> {
    let mut files: Vec<ChangedFile> = Vec::new();
    let mut current: Option<ChangedFile> = None;
    let mut in_file_header = false;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            files.extend(current.take());
            in_file_header = true;
        } else if in_file_header && let Some(path) = line.strip_prefix("+++ ") {
            current = new_path(path).map(|path| ChangedFile {
                path,
                changes: Vec::new(),
            });
        } else if let Some(header) = line.strip_prefix("@@ ") {
            in_file_header = false;
            if let Some(file) = current.as_mut()
                && let Some(range) = parse_hunk_header(header)
            {
                file.changes.push(range);
            }
        }
    }
    files.extend(current);
    files.retain(|file| !file.changes.is_empty());
    files
}

fn new_path(header_path: &str) -> Option<String> {
    let header_path = header_path.strip_suffix('\t').unwrap_or(header_path);
    let path = if header_path.starts_with('"') {
        unquote(header_path)?
    } else {
        header_path.to_string()
    };
    path.strip_prefix("b/").map(str::to_string)
}

fn unquote(quoted: &str) -> Option<String> {
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
                let middle = escaped.next().filter(u8::is_ascii_digit)?;
                let low = escaped.next().filter(u8::is_ascii_digit)?;
                (high - b'0') * 64 + (middle - b'0') * 8 + (low - b'0')
            }
            other => other,
        });
    }
    Some(text_from_bytes(bytes))
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
            last: start + 1,
        }),
        _ => Some(LineRange {
            first: start,
            last: start + count - 1,
        }),
    }
}

#[cfg(test)]
mod tests;
