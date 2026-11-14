use std::error::Error;

use snafu::CleanedErrorText;

/// An error and every cause under it on one line, each cause's text said once.
///
/// A library error like reqwest's often keeps the real reason (DNS, TLS, a timeout) only in
/// its source, so printing the top error alone hides why it happened.
pub fn error_chain(error: &dyn Error) -> String {
    CleanedErrorText::new(error)
        .map(|(_, text, _)| text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(": ")
}

#[cfg(test)]
mod tests;
