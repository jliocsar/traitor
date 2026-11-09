use std::env;
use std::error::Error;
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;

use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::cache::VerdictCache;
use traitor_core::check::check;
use traitor_core::configured_repository::ConfiguredRepository;
use traitor_core::git::Repository;
use traitor_core::stop_hook::{BlockedFails, StopHookInput, StopHookOutput, respond};
use traitor_core::store::JsonStore;
use traitor_core::trait_definition::TRAITS_DIRECTORY;

pub async fn run(revision: Option<&str>) -> ExitCode {
    let mut stdin = String::new();
    let input: StopHookInput = io::stdin()
        .read_to_string(&mut stdin)
        .ok()
        .and_then(|_| serde_json::from_str(&stdin).ok())
        .unwrap_or_default();
    let output = respond_to_stop(input, revision).await;
    println!(
        "{}",
        serde_json::to_string(&output).expect("hook output always serializes")
    );
    ExitCode::SUCCESS
}

async fn respond_to_stop(input: StopHookInput, revision: Option<&str>) -> StopHookOutput {
    let directory = match input.cwd.clone().map_or_else(env::current_dir, Ok) {
        Ok(directory) => directory,
        Err(error) => return StopHookOutput::could_not_check(&error.to_string()),
    };
    let has_traits = Repository::discover(&directory)
        .is_ok_and(|repository| repository.root().join(TRAITS_DIRECTORY).is_dir());
    if !has_traits {
        return StopHookOutput::default();
    }
    check_for_stop(&directory, &input, revision)
        .await
        .unwrap_or_else(|error| StopHookOutput::could_not_check(&error.to_string()))
}

async fn check_for_stop(
    directory: &Path,
    input: &StopHookInput,
    revision: Option<&str>,
) -> Result<StopHookOutput, Box<dyn Error>> {
    let repository = ConfiguredRepository::open(directory)?;
    let store = JsonStore::in_user_cache_directory();
    let report = check(
        &repository,
        revision,
        &TypeSafeJudge::from_environment(),
        &VerdictCache::new(&store),
    )
    .await?;
    let blocked_fails = BlockedFails::new(&store);
    let session_id = input.session_id.as_deref().unwrap_or_default();
    let (output, now_failing) = respond(&report, &blocked_fails.read(session_id));
    blocked_fails.write(session_id, &now_failing);
    Ok(output)
}
