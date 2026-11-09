use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::cache::VerdictCache;
use traitor_core::check::check;
use traitor_core::git::Repository;
use traitor_core::stop_hook::{BlockedFails, StopHookInput, StopHookOutput, respond};
use traitor_core::trait_definition::TRAITS_DIRECTORY;

use crate::run_in;

pub fn run_stop_hook(revision: Option<&str>) -> ExitCode {
    let mut stdin = String::new();
    let input: StopHookInput = io::stdin()
        .read_to_string(&mut stdin)
        .ok()
        .and_then(|_| serde_json::from_str(&stdin).ok())
        .unwrap_or_default();
    let output = respond_to_stop(input, revision);
    println!(
        "{}",
        serde_json::to_string(&output).expect("hook output always serializes")
    );
    ExitCode::SUCCESS
}

fn respond_to_stop(input: StopHookInput, revision: Option<&str>) -> StopHookOutput {
    let directory = match input
        .cwd
        .map_or_else(std::env::current_dir, Ok::<PathBuf, _>)
    {
        Ok(directory) => directory,
        Err(error) => return StopHookOutput::could_not_check(&error.to_string()),
    };
    let has_traits = Repository::discover(&directory)
        .is_ok_and(|repository| repository.root().join(TRAITS_DIRECTORY).is_dir());
    if !has_traits {
        return StopHookOutput::default();
    }
    let judge = TypeSafeJudge::from_environment();
    let cache = VerdictCache::in_user_cache_directory();
    match run_in(&directory, async |directory| {
        check(directory, revision, &judge, &cache).await
    }) {
        Ok(report) => {
            let blocked_fails = BlockedFails::in_user_cache_directory();
            let session_id = input.session_id.unwrap_or_default();
            let (output, now_failing) = respond(&report, &blocked_fails.read(&session_id));
            let _ = blocked_fails.write(&session_id, &now_failing);
            output
        }
        Err(message) => StopHookOutput::could_not_check(&message),
    }
}
