use std::fs::{self, DirEntry};
use std::io;
use std::path::{Path, PathBuf};

use globset::GlobSet;
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, Snafu};

use crate::glob::{GlobError, compile_globs};

pub const TRAITS_DIRECTORY: &str = ".agents/traits";
pub const TRAIT_FILE: &str = "TRAIT.md";

#[derive(Debug, Snafu)]
pub enum TraitDefinitionError {
    #[snafu(display("couldn't list {}: {source}", directory.display()))]
    ListTraits {
        directory: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("couldn't read {}: {source}", path.display()))]
    ReadTrait {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("{} needs YAML frontmatter between two --- lines", path.display()))]
    MissingFrontmatter { path: PathBuf },
    #[snafu(display("{} has invalid frontmatter: {source}", path.display()))]
    InvalidFrontmatter {
        path: PathBuf,
        #[snafu(source(from(serde_saphyr::Error, Box::new)))]
        source: Box<serde_saphyr::Error>,
    },
    #[snafu(transparent)]
    InvalidGlob { source: GlobError },
}

#[derive(Debug, Clone)]
pub struct TraitDefinition {
    pub slug: String,
    pub title: String,
    pub why: String,
    pub expect: String,
    paths: GlobSet,
    directory: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frontmatter {
    title: String,
    why: String,
    expect: String,
    paths: Vec<String>,
}

impl TraitDefinition {
    pub fn parse(slug: &str, path: &Path, contents: &str) -> Result<Self, TraitDefinitionError> {
        let frontmatter = split_frontmatter(contents).context(MissingFrontmatterSnafu { path })?;
        let frontmatter: Frontmatter =
            serde_saphyr::from_str(frontmatter).context(InvalidFrontmatterSnafu { path })?;
        Ok(Self {
            slug: slug.to_string(),
            title: frontmatter.title,
            why: frontmatter.why,
            expect: frontmatter.expect,
            paths: compile_globs(path, &frontmatter.paths)?,
            directory: path.parent().map(Path::to_path_buf).unwrap_or_default(),
        })
    }

    pub fn label(&self) -> String {
        format!("{} ({})", self.title, self.slug)
    }

    pub(crate) fn directory(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn applies_to(&self, path: &str) -> bool {
        self.paths.is_match(path)
    }
}

pub(crate) fn load_trait_definitions(
    repository_root: &Path,
) -> Result<Vec<TraitDefinition>, TraitDefinitionError> {
    let directory = repository_root.join(TRAITS_DIRECTORY);
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    visible_entries(&directory)
        .context(ListTraitsSnafu {
            directory: &directory,
        })?
        .iter()
        .filter(|entry| entry.path().is_dir())
        .map(|folder| {
            let slug = folder.file_name().to_string_lossy().into_owned();
            let path = folder.path().join(TRAIT_FILE);
            let contents = fs::read_to_string(&path).context(ReadTraitSnafu { path: &path })?;
            TraitDefinition::parse(&slug, &path, &contents)
        })
        .collect()
}

pub(crate) fn visible_entries(directory: &Path) -> io::Result<Vec<DirEntry>> {
    let mut entries: Vec<DirEntry> = fs::read_dir(directory)?.collect::<io::Result<_>>()?;
    entries.retain(|entry| !entry.file_name().to_string_lossy().starts_with('.'));
    entries.sort_by_key(DirEntry::file_name);
    Ok(entries)
}

fn split_frontmatter(contents: &str) -> Option<&str> {
    let rest = contents
        .strip_prefix("---\n")
        .or_else(|| contents.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some(&rest[..offset]);
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests;
