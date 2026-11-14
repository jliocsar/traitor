use std::path::Path;

use super::*;

#[test]
fn reads_the_session_and_directory_claude_code_sends() {
    let input =
        read_input(&br#"{"session_id":"abc","cwd":"/work","hook_event_name":"Stop"}"#[..]).unwrap();
    assert_eq!(input.session_id, "abc");
    assert_eq!(input.cwd, Path::new("/work"));
}

#[test]
fn input_without_a_session_is_an_error_not_a_shared_default() {
    let error = read_input(&br#"{"cwd":"/work"}"#[..]).unwrap_err();
    assert!(matches!(error, InputError::Parse { .. }));
}

#[test]
fn input_that_isnt_json_says_why_it_couldnt_check() {
    let error = read_input(&b"not json"[..]).unwrap_err();
    let message = StopHookOutput::could_not_check(&error)
        .system_message
        .unwrap();
    assert!(
        message.starts_with(
            "traitor couldn't check: the Stop hook's input isn't what Claude Code sends: expected"
        ),
        "{message}"
    );
}
