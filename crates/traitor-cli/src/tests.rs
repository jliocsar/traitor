//! Tests for telling a run as a Claude Code hook apart, before the arguments parse.

use super::*;

fn arguments(line: &str) -> impl Iterator<Item = String> {
    line.split(' ')
        .map(str::to_string)
        .collect::<Vec<_>>()
        .into_iter()
}

#[test]
fn the_hook_format_is_found_in_either_spelling() {
    assert!(runs_as_hook(arguments(
        "traitor check --format claude-code-hook"
    )));
    assert!(runs_as_hook(arguments(
        "traitor check --format=claude-code-hook"
    )));
}

#[test]
fn a_plain_check_isnt_a_hook() {
    assert!(!runs_as_hook(arguments("traitor check main..HEAD")));
}
