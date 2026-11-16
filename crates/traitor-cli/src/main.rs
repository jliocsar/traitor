//! The `traitor` command line: parses arguments, runs a command and maps its outcome to an exit code.

mod check_command;
mod error;
mod init_command;
mod session;
mod stop_hook_command;
mod test_command;

use std::env;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

use crate::error::{CliError, error_chain};

const EXIT_TRAIT_BROKEN: u8 = 1;
const EXIT_COULD_NOT_RUN: u8 = 2;
// A Stop hook's exit 2 keeps Claude working, with stderr as the reason it reads.
const EXIT_BLOCK_STOP: u8 = 2;
// Exit 1 is a non-blocking error for a Claude Code hook: the user sees it, and Claude may stop.
// A hook that can't run must never exit 2, or every stop would block on an error Claude can't fix.
const EXIT_HOOK_COULD_NOT_RUN: u8 = 1;
const HOOK_FORMAT: &str = "claude-code-hook";

#[derive(Parser)]
#[command(name = "traitor", version, about)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Text,
    ClaudeCodeHook,
}

#[derive(Subcommand)]
enum Command {
    #[command(
        about = "Set traitor up in this repo, or upgrade its config to this traitor's version. Setting up is how a repo agrees to send its code to TypeSafe."
    )]
    Init,
    /// Judge every changed snippet against the traits whose paths match it.
    Check {
        /// What to judge, as `git diff` reads it: nothing for everything unpushed, a ref for everything since it, or A..B for what changed between two commits.
        revision: Option<String>,
        /// How to report: text for people and CI, or claude-code-hook for the JSON a Claude Code Stop hook reads on stdin and prints.
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
    /// Judge each trait's cases, the files in its pass/ and fail/ folders, and report any that get a different verdict.
    Test {
        /// The traits to test, by folder name. Every trait when none are given.
        traits: Vec<String>,
    },
}

impl Command {
    fn is_hook(&self) -> bool {
        matches!(
            self,
            Self::Check {
                format: Format::ClaudeCodeHook,
                ..
            }
        )
    }
}

enum Outcome {
    Success,
    TraitBroken,
    BlockStop,
}

type CommandResult = Result<Outcome, CliError>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let command = match Arguments::try_parse() {
        Ok(arguments) => arguments.command,
        Err(error) if error.use_stderr() && runs_as_hook(env::args()) => {
            eprint!("{error}");

            return ExitCode::from(EXIT_HOOK_COULD_NOT_RUN);
        }
        Err(error) => error.exit(),
    };

    let could_not_run = if command.is_hook() {
        EXIT_HOOK_COULD_NOT_RUN
    } else {
        EXIT_COULD_NOT_RUN
    };

    let (activity, result) = match command {
        Command::Init => ("set up", init_command::run()),
        Command::Check {
            revision,
            format: Format::Text,
        } => ("check", check_command::run(revision.as_deref()).await),
        Command::Check {
            revision,
            format: Format::ClaudeCodeHook,
        } => ("check", stop_hook_command::run(revision.as_deref()).await),
        Command::Test { traits } => ("test", test_command::run(&traits).await),
    };

    match result {
        Ok(Outcome::Success) => ExitCode::SUCCESS,
        Ok(Outcome::TraitBroken) => ExitCode::from(EXIT_TRAIT_BROKEN),
        Ok(Outcome::BlockStop) => ExitCode::from(EXIT_BLOCK_STOP),
        Err(error) => {
            eprintln!("traitor couldn't {activity}: {}", error_chain(&error));

            ExitCode::from(could_not_run)
        }
    }
}

/// Whether the raw arguments ask for the hook format, for when they don't parse.
fn runs_as_hook(mut arguments: impl Iterator<Item = String>) -> bool {
    arguments.any(|argument| {
        argument == HOOK_FORMAT || argument.strip_prefix("--format=") == Some(HOOK_FORMAT)
    })
}

#[cfg(test)]
mod tests;
