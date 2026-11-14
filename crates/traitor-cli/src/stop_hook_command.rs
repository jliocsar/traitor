use std::error::Error;
use std::io::{self, Read};

use snafu::{ResultExt, Snafu};
use traitor_ai_typesafe::TypeSafeJudge;
use traitor_core::stop_hook::{StopHookInput, StopHookOutput, respond_to_stop};
use traitor_core::store::JsonStore;

use crate::{CommandResult, Outcome};

#[derive(Debug, Snafu)]
enum InputError {
    #[snafu(display("couldn't read the Stop hook's input: {source}"))]
    Read { source: io::Error },
    #[snafu(display("the Stop hook's input isn't what Claude Code sends: {source}"))]
    Parse { source: serde_json::Error },
}

pub async fn run(revision: Option<&str>) -> CommandResult {
    let output = match read_input(io::stdin()) {
        Ok(input) => respond(input, revision)
            .await
            .unwrap_or_else(|error| StopHookOutput::could_not_check(&*error)),
        Err(error) => StopHookOutput::could_not_check(&error),
    };
    println!(
        "{}",
        serde_json::to_string(&output).expect("hook output always serializes")
    );
    Ok(Outcome::Success)
}

fn read_input(mut reader: impl Read) -> Result<StopHookInput, InputError> {
    let mut text = String::new();
    reader.read_to_string(&mut text).context(ReadSnafu)?;
    serde_json::from_str(&text).context(ParseSnafu)
}

async fn respond(
    input: StopHookInput,
    revision: Option<&str>,
) -> Result<StopHookOutput, Box<dyn Error>> {
    let judge = TypeSafeJudge::from_environment()?;
    Ok(respond_to_stop(
        &input.cwd,
        &input.session_id,
        revision,
        &judge,
        &JsonStore::in_user_cache_directory(),
    )
    .await)
}

#[cfg(test)]
mod tests;
