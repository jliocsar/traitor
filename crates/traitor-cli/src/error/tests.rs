use std::io;

use snafu::Snafu;

use super::*;

#[derive(Debug, Snafu)]
#[snafu(display("couldn't reach TypeSafe: {source}"))]
struct Reach {
    source: SendRequest,
}

#[derive(Debug, Snafu)]
#[snafu(display("error sending request"))]
struct SendRequest {
    source: io::Error,
}

#[derive(Debug, Snafu)]
#[snafu(display("{source}"))]
struct Wrapper {
    source: Reach,
}

fn reach() -> Reach {
    Reach {
        source: SendRequest {
            source: io::Error::other("dns error: no such host"),
        },
    }
}

#[test]
fn the_chain_shows_the_cause_under_an_error_that_hides_it() {
    assert_eq!(
        error_chain(&reach()),
        "couldn't reach TypeSafe: error sending request: dns error: no such host"
    );
}

#[test]
fn an_error_that_only_repeats_its_source_adds_nothing() {
    assert_eq!(
        error_chain(&Wrapper { source: reach() }),
        "couldn't reach TypeSafe: error sending request: dns error: no such host"
    );
}
