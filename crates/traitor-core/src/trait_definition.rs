use std::fs;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use snafu::{OptionExt, ResultExt, Snafu};

pub const TRAITS_DIRECTORY: &str = ".agents/traits";
const TRAIT_FILE: &str = "TRAIT.md";

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
    #[snafu(display("{} has an invalid glob in paths, {glob:?}: {source}", path.display()))]
    InvalidGlob {
        path: PathBuf,
        glob: String,
        source: globset::Error,
    },
}

#[derive(Debug, Clone)]
pub struct TraitDefinition {
    pub slug: String,
    pub title: String,
    pub why: String,
    pub expect: String,
    paths: GlobSet,
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
        })
    }

    pub fn applies_to(&self, path: &str) -> bool {
        self.paths.is_match(path)
    }
}

pub fn load_trait_definitions(
    repository_root: &Path,
) -> Result<Vec<TraitDefinition>, TraitDefinitionError> {
    let directory = repository_root.join(TRAITS_DIRECTORY);
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut folders = fs::read_dir(&directory)
        .and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
        .context(ListTraitsSnafu {
            directory: &directory,
        })?
        .into_iter()
        .filter(|entry| entry.path().is_dir())
        .collect::<Vec<_>>();
    folders.sort_by_key(|entry| entry.file_name());
    folders
        .iter()
        .map(|folder| {
            let slug = folder.file_name().to_string_lossy().into_owned();
            let path = folder.path().join(TRAIT_FILE);
            let contents = fs::read_to_string(&path).context(ReadTraitSnafu { path: &path })?;
            TraitDefinition::parse(&slug, &path, &contents)
        })
        .collect()
}

pub fn compile_globs(path: &Path, globs: &[String]) -> Result<GlobSet, TraitDefinitionError> {
    globs
        .iter()
        .try_fold(GlobSetBuilder::new(), |mut builder, glob| {
            builder.add(
                GlobBuilder::new(glob)
                    .literal_separator(true)
                    .build()
                    .context(InvalidGlobSnafu { path, glob })?,
            );
            Ok(builder)
        })?
        .build()
        .context(InvalidGlobSnafu {
            path,
            glob: globs.join(", "),
        })
}

fn split_frontmatter(contents: &str) -> Option<&str> {
    let rest = contents
        .strip_prefix("---\n")
        .or_else(|| contents.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    Some(&rest[..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAILWAY: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\nexpect: A computed value flows through combinators.\npaths:\n  - \"src/**/*.ts\"\n---\n\nSome prose that isn't sent.\n\n## Pass\n\n```ts\nconst label = Option.map(name, toTitle)\n```\n\n## Fail\n\n````ts\nlet label = ''\n```\nnot a close\n```\n````\n\n```ts\nlet total = 0\n```\n";

    #[test]
    fn reads_the_frontmatter_and_ignores_the_body() {
        let definition =
            TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), RAILWAY).unwrap();
        assert_eq!(definition.title, "Railway, not guards");
        assert_eq!(
            definition.expect,
            "A computed value flows through combinators."
        );
    }

    #[test]
    fn paths_match_with_literal_separators() {
        let definition =
            TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), RAILWAY).unwrap();
        assert!(definition.applies_to("src/module.ts"));
        assert!(definition.applies_to("src/deep/module.ts"));
        assert!(!definition.applies_to("test/module.ts"));
        assert!(!definition.applies_to("src/module.tsx"));
    }

    #[test]
    fn rejects_unknown_frontmatter_fields() {
        let contents = RAILWAY.replace("paths:", "kind: code\npaths:");
        assert!(matches!(
            TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), &contents),
            Err(TraitDefinitionError::InvalidFrontmatter { .. })
        ));
    }
}
