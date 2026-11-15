//! Tests for opening a repository, and for telling "no repository" from "git won't run".

use std::fs;

use tempfile::TempDir;

use super::*;
use crate::test_support::{git, repository_with_trait};

#[test]
fn git_that_wont_run_is_an_error_not_no_repository() {
    let missing_git = GitError::Spawn {
        source: std::io::Error::from(std::io::ErrorKind::NotFound),
    };

    assert!(matches!(
        repository_with_traits(Err(missing_git)),
        Err(GitError::Spawn { .. })
    ));
}

#[test]
fn a_directory_outside_any_repository_has_no_repository() {
    let not_a_repository = GitError::Failed {
        arguments: "rev-parse --show-toplevel".to_string(),
        stderr: "fatal: not a git repository".to_string(),
    };

    assert!(matches!(
        repository_with_traits(Err(not_a_repository)),
        Ok(None)
    ));
}

#[test]
fn a_repo_without_traits_opens_as_none() {
    let directory = TempDir::new().unwrap();

    git(directory.path(), &["init", "-q"]);

    assert!(matches!(
        ConfiguredRepository::open_if_it_has_traits(directory.path()),
        Ok(None)
    ));
}

#[test]
fn a_repo_with_traits_but_no_config_is_an_error() {
    let directory = repository_with_trait("{}");

    fs::remove_file(directory.path().join(".agents/traitor.json")).unwrap();

    assert!(matches!(
        ConfiguredRepository::open_if_it_has_traits(directory.path()),
        Err(OpenError::Config {
            source: ConfigError::MissingConfig { .. }
        })
    ));
}
