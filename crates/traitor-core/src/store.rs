//! A directory of JSON files, keyed by name, for the verdict cache and the Stop hook's blocked fails.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use snafu::{ResultExt, Snafu};

const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// A cache file that couldn't be written. Nothing is lost but time: a later run asks again.
#[derive(Debug, Snafu)]
#[snafu(display("couldn't write {}: {source}", path.display()))]
pub struct CacheWriteError {
    path: PathBuf,
    source: io::Error,
}

#[derive(Clone)]
pub struct JsonStore {
    directory: Option<PathBuf>,
}

impl JsonStore {
    #[must_use]
    pub fn in_user_cache_directory() -> Self {
        Self {
            directory: dirs::cache_dir().map(|cache| cache.join("traitor").join("v1")),
        }
    }

    #[cfg(test)]
    pub(crate) fn at(directory: PathBuf) -> Self {
        Self {
            directory: Some(directory),
        }
    }

    pub(crate) fn subdirectory(&self, name: &str) -> Self {
        Self {
            directory: self
                .directory
                .as_ref()
                .map(|directory| directory.join(name)),
        }
    }

    pub(crate) fn read<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let contents = fs::read(self.directory.as_ref()?.join(format!("{name}.json"))).ok()?;
        serde_json::from_slice(&contents).ok()
    }

    /// Writes `value` as `name`. A store without a directory writes nothing.
    pub(crate) fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<(), CacheWriteError> {
        let Some(directory) = &self.directory else {
            return Ok(());
        };

        write_atomically(directory, name, value).context(CacheWriteSnafu {
            path: directory.join(format!("{name}.json")),
        })
    }
}

fn write_atomically<T: Serialize>(directory: &Path, name: &str, value: &T) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let temporary = directory.join(format!("{name}.json.{}.tmp", process::id()));
    fs::write(&temporary, serde_json::to_vec(value)?)?;
    fs::rename(&temporary, directory.join(format!("{name}.json")))
}

pub(crate) fn hex_sha256(material: &[u8]) -> String {
    Sha256::digest(material)
        .iter()
        .flat_map(|byte| {
            [
                HEX_DIGITS[usize::from(byte >> 4)],
                HEX_DIGITS[usize::from(byte & 0x0f)],
            ]
        })
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests;
