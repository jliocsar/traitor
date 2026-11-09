mod check_command;
mod init_command;
mod stop_hook_command;
mod test_command;

use std::error::Error;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

const EXIT_TRAIT_BROKEN: u8 = 1;
const EXIT_COULD_NOT_RUN: u8 = 2;

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
    /// Set traitor up in this repo, or upgrade its config to this traitor's version. Setting up is how a repo agrees to send its code to TypeSafe.
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

enum Outcome {
    Success,
    TraitBroken,
}

type CommandResult = Result<Outcome, Box<dyn Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let (activity, result) = match Arguments::parse().command {
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
        Err(error) => {
            eprintln!("traitor couldn't {activity}: {error}");
            ExitCode::from(EXIT_COULD_NOT_RUN)
        }
    }
}
