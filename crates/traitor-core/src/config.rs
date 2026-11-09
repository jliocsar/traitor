use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use globset::GlobSet;
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(rename = "$schema", default)]
    _schema: Option<String>,
    #[serde(default)]
    traits: BTreeMap<String, TraitSettingsFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitSettingsFile {
    #[serde(default)]
    threshold: Option<f64>,
    #[serde(default)]
    exempt: Vec<ExemptionFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExemptionFile {
    paths: Vec<String>,
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
