use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use globset::GlobSet;
use schemars::JsonSchema;
use serde::Deserialize;
use snafu::{ResultExt, Snafu, ensure};

use crate::trait_definition::{TraitDefinitionError, compile_globs};

pub const CONFIG_PATH: &str = ".agents/traitor.json";

#[derive(Debug, Snafu)]
pub enum ConfigError {
    #[snafu(display(
        "{} doesn't exist. Run `traitor init` to set traitor up here; that's also how a repo agrees to send its code to TypeSafe.",
        path.display()
    ))]
    MissingConfig { path: PathBuf },
    #[snafu(display("couldn't read {}: {source}", path.display()))]
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("{} is invalid: {source}", path.display()))]
    InvalidConfig {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[snafu(display(
        "{} gives traits.{slug} a threshold of {threshold}, but a threshold is a probability between 0 and 1",
        path.display()
    ))]
    ThresholdOutOfRange {
        path: PathBuf,
        slug: String,
        threshold: f64,
    },
    #[snafu(display("{} exempts paths from traits.{slug} without a reason", path.display()))]
    MissingReason { path: PathBuf, slug: String },
    #[snafu(display("{source}"))]
    InvalidExemption { source: TraitDefinitionError },
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    title = "traitor config",
    description = "A repo's traitor settings, in .agents/traitor.json. Run `traitor init` to create or upgrade it."
)]
struct ConfigFile {
    #[serde(rename = "$schema", default)]
    #[schemars(
        description = "The schema for the traitor version this config is for. `traitor init` reads the version from it and keeps it current."
    )]
    _schema: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Settings for each trait, by the name of its folder in .agents/traits."
    )]
    traits: BTreeMap<String, TraitSettingsFile>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "TraitSettings")]
struct TraitSettingsFile {
    #[serde(default)]
    #[schemars(
        range(min = 0.0, max = 1.0),
        description = "The probability of a fail at or above which this trait fails the check. Without one, the trait's verdicts are only reported."
    )]
    threshold: Option<f64>,
    #[serde(default)]
    #[schemars(description = "Paths this trait doesn't judge, each group with its reason.")]
    exempt: Vec<ExemptionFile>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "Exemption")]
struct ExemptionFile {
    #[schemars(
        description = "Globs relative to the repo root, where * stops at a slash and ** crosses them."
    )]
    paths: Vec<String>,
    #[schemars(
        pattern(r"\S"),
        description = "Why these paths don't have to hold the trait."
    )]
    reason: String,
}

pub struct Config {
    traits: BTreeMap<String, TraitPolicy>,
}

struct TraitPolicy {
    threshold: Option<f64>,
    exempt: GlobSet,
}

impl Config {
    pub fn load(repository_root: &Path) -> Result<Self, ConfigError> {
        let path = repository_root.join(CONFIG_PATH);
        let contents = fs::read(&path).map_err(|source| match source.kind() {
            ErrorKind::NotFound => ConfigError::MissingConfig { path: path.clone() },
            _ => ConfigError::ReadConfig {
                path: path.clone(),
                source,
            },
        })?;
        Self::parse(&path, &contents)
    }

    pub fn parse(path: &Path, contents: &[u8]) -> Result<Self, ConfigError> {
        let file: ConfigFile =
            serde_json::from_slice(contents).context(InvalidConfigSnafu { path })?;
        let traits = file
            .traits
            .into_iter()
            .map(|(slug, settings)| {
                let policy = TraitPolicy::from_settings(path, &slug, settings)?;
                Ok((slug, policy))
            })
            .collect::<Result<_, ConfigError>>()?;
        Ok(Self { traits })
    }

    pub fn threshold(&self, slug: &str) -> Option<f64> {
        self.traits.get(slug).and_then(|policy| policy.threshold)
    }

    pub fn is_exempt(&self, slug: &str, path: &str) -> bool {
        self.traits
            .get(slug)
            .is_some_and(|policy| policy.exempt.is_match(path))
    }

    pub fn configured_traits(&self) -> impl Iterator<Item = &str> {
        self.traits.keys().map(String::as_str)
    }
}

pub fn config_json_schema() -> String {
    let schema = schemars::generate::SchemaSettings::draft2020_12()
        .into_generator()
        .into_root_schema_for::<ConfigFile>();
    let mut text = serde_json::to_string_pretty(&schema).expect("a schema always serializes");
    text.push('\n');
    text
}

impl TraitPolicy {
    fn from_settings(
        path: &Path,
        slug: &str,
        settings: TraitSettingsFile,
    ) -> Result<Self, ConfigError> {
        if let Some(threshold) = settings.threshold {
            ensure!(
                (0.0..=1.0).contains(&threshold),
                ThresholdOutOfRangeSnafu {
                    path,
                    slug,
                    threshold
                }
            );
        }
        ensure!(
            settings
                .exempt
                .iter()
                .all(|exemption| !exemption.reason.trim().is_empty()),
            MissingReasonSnafu { path, slug }
        );
        let globs: Vec<String> = settings
            .exempt
            .into_iter()
            .flat_map(|exemption| exemption.paths)
            .collect();
        Ok(Self {
            threshold: settings.threshold,
            exempt: compile_globs(path, &globs).context(InvalidExemptionSnafu)?,
        })
    }
}

#[cfg(test)]
mod tests;
