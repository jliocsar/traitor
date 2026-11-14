//! Everything a command needs to judge a repository: the repository, the judge and the cache.

use std::env;
use std::path::Path;

use snafu::ResultExt;
use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::configured_repository::ConfiguredRepository;
use traitor_core::store::JsonStore;

use crate::error::{CliError, CurrentDirectorySnafu};

/// A repository set up to be judged: the repository, the judge, and the cache verdicts go through.
pub(crate) struct Session {
    pub(crate) repository: ConfiguredRepository,
    pub(crate) judge: TypeSafeJudge,
    pub(crate) store: JsonStore,
}

impl Session {
    pub(crate) fn in_current_directory() -> Result<Self, CliError> {
        Self::judging(ConfiguredRepository::open(
            &env::current_dir().context(CurrentDirectorySnafu)?,
        )?)
    }

    /// The session for `directory`, or none when it isn't in a git repository with traits.
    pub(crate) fn if_it_has_traits(directory: &Path) -> Result<Option<Self>, CliError> {
        ConfiguredRepository::open_if_it_has_traits(directory)?
            .map(Self::judging)
            .transpose()
    }

    fn judging(repository: ConfiguredRepository) -> Result<Self, CliError> {
        Ok(Self {
            repository,
            judge: TypeSafeJudge::from_environment()?,
            store: JsonStore::in_user_cache_directory(),
        })
    }
}
