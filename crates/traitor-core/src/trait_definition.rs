use std::fs;
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Examples {
    pub pass: Vec<String>,
    pub fail: Vec<String>,
}

impl Examples {
    pub fn is_empty(&self) -> bool {
        self.pass.is_empty() && self.fail.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct TraitDefinition {
    pub slug: String,
    pub title: String,
    pub why: String,
    pub expect: String,
    pub examples: Examples,
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

#[derive(Clone, Copy)]
enum ExampleKind {
    Pass,
    Fail,
}

impl TraitDefinition {
    pub fn parse(slug: &str, path: &Path, contents: &str) -> Result<Self, TraitDefinitionError> {
        let (frontmatter, body) =
            split_frontmatter(contents).context(MissingFrontmatterSnafu { path })?;
        let frontmatter: Frontmatter =
            serde_saphyr::from_str(frontmatter).context(InvalidFrontmatterSnafu { path })?;
        Ok(Self {
            slug: slug.to_string(),
            title: frontmatter.title,
            why: frontmatter.why,
            expect: frontmatter.expect,
            examples: parse_examples(body),
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

fn split_frontmatter(contents: &str) -> Option<(&str, &str)> {
    let rest = contents
        .strip_prefix("---\n")
        .or_else(|| contents.strip_prefix("---\r\n"))?;
    let end = rest.find("\n---")?;
    let after = &rest[end + 4..];
    let body = after.split_once('\n').map_or("", |(_, body)| body);
    Some((&rest[..=end], body))
}

fn parse_examples(body: &str) -> Examples {
    let mut examples = Examples::default();
    let mut section = None;
    let mut fence: Option<(String, Vec<&str>)> = None;
    for line in body.lines() {
        if let Some((marker, lines)) = &mut fence {
            if closes_fence(line, marker) {
                let example = lines.join("\n");
                match section {
                    Some(ExampleKind::Pass) => examples.pass.push(example),
                    Some(ExampleKind::Fail) => examples.fail.push(example),
                    None => {}
                }
                fence = None;
            } else {
                lines.push(line);
            }
        } else if let Some(marker) = opening_fence(line) {
            fence = Some((marker, Vec::new()));
        } else if let Some(heading) = heading_text(line) {
            section = match heading.to_lowercase().as_str() {
                "pass" => Some(ExampleKind::Pass),
                "fail" => Some(ExampleKind::Fail),
                _ => None,
            };
        }
    }
    examples
}

fn heading_text(line: &str) -> Option<&str> {
    let text = line.trim_start_matches('#');
    (text.len() < line.len() && text.starts_with(' ')).then(|| text.trim())
}

fn opening_fence(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let character = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let marker: String = trimmed.chars().take_while(|c| *c == character).collect();
    (marker.len() >= 3).then_some(marker)
}

fn closes_fence(line: &str, marker: &str) -> bool {
    let trimmed = line.trim();
    let character = marker.chars().next().unwrap_or('`');
    trimmed.len() >= marker.len() && trimmed.chars().all(|c| c == character)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAILWAY: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\nexpect: A computed value flows through combinators.\npaths:\n  - \"src/**/*.ts\"\n---\n\nSome prose that isn't sent.\n\n## Pass\n\n```ts\nconst label = Option.map(name, toTitle)\n```\n\n## Fail\n\n````ts\nlet label = ''\n```\nnot a close\n```\n````\n\n```ts\nlet total = 0\n```\n";

    #[test]
    fn parses_frontmatter_and_examples() {
        let definition =
            TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), RAILWAY).unwrap();
        assert_eq!(definition.title, "Railway, not guards");
        assert_eq!(
            definition.examples,
            Examples {
                pass: vec!["const label = Option.map(name, toTitle)".to_string()],
                fail: vec![
                    "let label = ''\n```\nnot a close\n```".to_string(),
                    "let total = 0".to_string()
                ],
            }
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
