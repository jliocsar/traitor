use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct JsonStore {
    directory: Option<PathBuf>,
}

impl JsonStore {
    pub fn in_user_cache_directory() -> Self {
        Self {
            directory: dirs::cache_dir().map(|cache| cache.join("traitor").join("v1")),
        }
    }

    pub fn at(directory: PathBuf) -> Self {
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

    pub(crate) fn write_best_effort<T: Serialize>(&self, name: &str, value: &T) {
        if let Some(directory) = &self.directory {
            let _ = write_atomically(directory, name, value);
        }
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
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}
