use std::env;
use std::error::Error;
use std::io::{self, Read};
use std::process::ExitCode;

use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::stop_hook::{StopHookInput, StopHookOutput, respond_to_stop};
use traitor_core::store::JsonStore;

pub async fn run(revision: Option<&str>) -> ExitCode {
    let mut stdin = String::new();
    let input: StopHookInput = io::stdin()
        .read_to_string(&mut stdin)
        .ok()
        .and_then(|_| serde_json::from_str(&stdin).ok())
        .unwrap_or_default();
    let output = respond(input, revision)
        .await
        .unwrap_or_else(|error| StopHookOutput::could_not_check(&error.to_string()));
    println!(
        "{}",
        serde_json::to_string(&output).expect("hook output always serializes")
    );
    ExitCode::SUCCESS
}

async fn respond(
    input: StopHookInput,
    revision: Option<&str>,
) -> Result<StopHookOutput, Box<dyn Error>> {
    let directory = input.cwd.map_or_else(env::current_dir, Ok)?;
    let judge = TypeSafeJudge::from_environment()?;
    Ok(respond_to_stop(
        &directory,
        input.session_id.as_deref().unwrap_or_default(),
        revision,
        &judge,
        &JsonStore::in_user_cache_directory(),
    )
    .await)
}
