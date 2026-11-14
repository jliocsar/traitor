//! Tests for how an answer is shown.

use super::*;

#[test]
fn an_answer_shows_as_its_serde_name() {
    for answer in [
        Answer::Pass,
        Answer::Fail,
        Answer::NotApplicable,
        Answer::Unsure,
    ] {
        assert_eq!(
            serde_json::to_string(&answer).unwrap(),
            format!("\"{answer}\"")
        );
    }
}
