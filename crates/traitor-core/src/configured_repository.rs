use std::path::Path;

use snafu::Snafu;

use crate::config::{CONFIG_PATH, Config, ConfigError};
use crate::git::{GitError, Repository};
use crate::trait_definition::{
    TRAITS_DIRECTORY, TraitDefinition, TraitDefinitionError, load_trait_definitions,
};
use crate::verdict::Verdict;

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

    pub(crate) fn load(git: Repository) -> Result<Self, OpenError> {
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

    pub(crate) fn fails_check(&self, definition: &TraitDefinition, verdict: &Verdict) -> bool {
        self.config.fails_check(&definition.slug, verdict)
    }
}
