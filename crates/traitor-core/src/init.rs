use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use semver::Version;
use serde_json::{Map, Value};
use snafu::{OptionExt, ResultExt, Snafu};

use crate::config::{
    CONFIG_PATH, Config, ConfigError, SCHEMA_KEY, new_config_file, read_if_present, schema_url,
    version_in_schema_url,
};
use crate::git::{GitError, Repository};

#[derive(Debug, Snafu)]
pub enum InitError {
    #[snafu(transparent)]
    Git { source: GitError },
    #[snafu(display("couldn't write {}: {source}", path.display()))]
    WriteConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    #[snafu(display("{} isn't valid JSON: {source}", path.display()))]
    InvalidJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[snafu(display("{} has to hold a JSON object", path.display()))]
    NotAnObject { path: PathBuf },
    #[snafu(display(
        "{} has a $schema of {url:?}, which isn't traitor's. Set it to {expected:?}, or remove it and run `traitor init` again.",
        path.display()
    ))]
    UnrecognizedSchema {
        path: PathBuf,
        url: String,
        expected: String,
    },
    #[snafu(display(
        "{} is for traitor {config_version}, newer than this traitor ({binary_version}). Update traitor instead.",
        path.display()
    ))]
    NewerThanBinary {
        path: PathBuf,
        config_version: Version,
        binary_version: Version,
    },
    #[snafu(transparent)]
    Config { source: ConfigError },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitOutcome {
    Created,
    AlreadyCurrent,
    Upgraded { from: Version },
    AddedSchema,
}

pub fn init(directory: &Path, binary_version: &Version) -> Result<InitOutcome, InitError> {
    let repository = Repository::discover(directory)?;
    let path = repository.root().join(CONFIG_PATH);
    if let Some(contents) = read_if_present(&path)? {
        return upgrade(&path, &contents, binary_version);
    }
    write_config(&path, &new_config_file(binary_version))?;
    Ok(InitOutcome::Created)
}

fn upgrade(
    path: &Path,
    contents: &[u8],
    binary_version: &Version,
) -> Result<InitOutcome, InitError> {
    let value: Value = serde_json::from_slice(contents).context(InvalidJsonSnafu { path })?;
    let Value::Object(mut config) = value else {
        return NotAnObjectSnafu { path }.fail();
    };
    let current_url = schema_url(binary_version);
    let outcome = match config.get(SCHEMA_KEY) {
        None => InitOutcome::AddedSchema,
        Some(url) => {
            let url = url.as_str().map_or_else(|| url.to_string(), str::to_string);
            let config_version = version_in_schema_url(&url).context(UnrecognizedSchemaSnafu {
                path,
                url,
                expected: &current_url,
            })?;
            match config_version.cmp(binary_version) {
                Ordering::Equal => {
                    Config::parse(path, contents)?;
                    return Ok(InitOutcome::AlreadyCurrent);
                }
                Ordering::Greater => {
                    return NewerThanBinarySnafu {
                        path,
                        config_version,
                        binary_version: binary_version.clone(),
                    }
                    .fail();
                }
                Ordering::Less => InitOutcome::Upgraded {
                    from: config_version,
                },
            }
        }
    };
    config.shift_insert(0, SCHEMA_KEY.to_string(), Value::String(current_url));
    write_config(path, &config)?;
    Ok(outcome)
}

fn write_config(path: &Path, config: &Map<String, Value>) -> Result<(), InitError> {
    let mut contents = serde_json::to_string_pretty(config).expect("a JSON map always serializes");
    contents.push('\n');
    Config::parse(path, contents.as_bytes())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context(WriteConfigSnafu { path })?;
    }
    fs::write(path, contents).context(WriteConfigSnafu { path })
}

#[cfg(test)]
mod tests;
