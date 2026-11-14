//! `traitor init`: sets traitor up in a repo or upgrades its config.

use std::env;

use semver::Version;
use snafu::ResultExt;
use traitor_ai_typesafe::API_HOST;
use traitor_core::config::CONFIG_PATH;
use traitor_core::init::{InitOutcome, init};
use traitor_core::trait_definition::{TRAIT_FILE, TRAITS_DIRECTORY};

use crate::error::{CurrentDirectorySnafu, OwnVersionSnafu};
use crate::{CommandResult, Outcome};

pub(crate) fn run() -> CommandResult {
    let version = Version::parse(env!("CARGO_PKG_VERSION")).context(OwnVersionSnafu)?;

    match init(
        &env::current_dir().context(CurrentDirectorySnafu)?,
        &version,
    )? {
        InitOutcome::Created => {
            println!("Wrote {CONFIG_PATH}.");
            println!();
            println!(
                "From now on, `traitor check` sends each changed snippet in this repo to TypeSafe ({API_HOST}) to be judged. TypeSafe says it doesn't train on what it receives, and keeps it for as long as is reasonably necessary."
            );
            println!(
                "Committing {CONFIG_PATH} is how this repo agrees to that, for everyone who works on it. Delete it to stop."
            );
            println!();
            println!("Next, add a trait as {TRAITS_DIRECTORY}/<slug>/{TRAIT_FILE}.");
        }
        InitOutcome::AlreadyCurrent => {
            println!("{CONFIG_PATH} is already on traitor {version}.");
        }
        InitOutcome::Upgraded { from } => {
            println!("Upgraded {CONFIG_PATH} from traitor {from} to {version}.");
        }
        InitOutcome::AddedSchema => {
            println!("Pinned {CONFIG_PATH} to traitor {version}'s schema.");
        }
    }

    Ok(Outcome::Success)
}
