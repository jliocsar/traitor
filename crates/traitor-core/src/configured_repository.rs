use std::path::Path;

use snafu::Snafu;

use crate::config::{CONFIG_PATH, Config, ConfigError};
use crate::git::{GitError, Repository};
use crate::trait_definition::{
    TRAITS_DIRECTORY, TraitDefinition, TraitDefinitionError, load_trait_definitions,
};

#[derive(Debug, Snafu)]
pub enum OpenError {
    #[snafu(transparent)]
    Git { source: GitError },
    #[snafu(transparent)]
    Config { source: ConfigError },
    #[snafu(transparent)]
    TraitDefinitions { source: TraitDefinitionError },
    #[snafu(display(
        "{CONFIG_PATH} configures traits.{slug}, but there's no {TRAITS_DIRECTORY}/{slug}"
    ))]
    UnknownTrait { slug: String },
}

#[derive(Debug)]
pub struct ConfiguredRepository {
    git: Repository,
    config: Config,
    definitions: Vec<TraitDefinition>,
}

impl ConfiguredRepository {
    pub fn open(directory: &Path) -> Result<Self, OpenError> {
        Self::load(Repository::discover(directory)?)
    }

    /// The repository at `directory`, or none when it isn't in a git repository or has no
    /// traits. Git failing to run at all is an error, not "no repository".
    pub fn open_if_it_has_traits(directory: &Path) -> Result<Option<Self>, OpenError> {
        repository_with_traits(Repository::discover(directory))?
            .map(Self::load)
            .transpose()
    }

    fn load(git: Repository) -> Result<Self, OpenError> {
        let config = Config::load(git.root())?;
        let definitions = load_trait_definitions(git.root())?;
        let repository = Self {
            git,
            config,
            definitions,
        };
        if let Some(slug) = repository
            .config
            .configured_traits()
            .find(|slug| repository.definition(slug).is_none())
        {
            return UnknownTraitSnafu { slug }.fail();
        }
        Ok(repository)
    }

    pub(crate) fn git(&self) -> &Repository {
        &self.git
    }

    pub(crate) fn definitions(&self) -> &[TraitDefinition] {
        &self.definitions
    }

    pub(crate) fn definition(&self, slug: &str) -> Option<&TraitDefinition> {
        self.definitions
            .iter()
            .find(|definition| definition.slug == slug)
    }

    pub(crate) fn traits_for(&self, path: &str) -> Vec<&TraitDefinition> {
        self.definitions
            .iter()
            .filter(|definition| {
                definition.applies_to(path) && !self.config.is_exempt(&definition.slug, path)
            })
            .collect()
    }

    pub(crate) fn threshold(&self, definition: &TraitDefinition) -> Option<f64> {
        self.config.threshold(&definition.slug)
    }
}

fn repository_with_traits(
    discovered: Result<Repository, GitError>,
) -> Result<Option<Repository>, GitError> {
    match discovered {
        Ok(git) => Ok(git.root().join(TRAITS_DIRECTORY).is_dir().then_some(git)),
        Err(GitError::Failed { .. }) => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
