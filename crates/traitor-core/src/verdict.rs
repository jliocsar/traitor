use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Answer {
    #[serde(rename = "pass")]
    Pass,
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "n/a")]
    NotApplicable,
    #[serde(rename = "unsure")]
    Unsure,
}

/// Shown as Jev names it, the same as its serde name.
impl fmt::Display for Answer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::NotApplicable => "n/a",
            Self::Unsure => "unsure",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Probabilities {
    pub pass: f64,
    pub fail: f64,
    #[serde(rename = "n/a")]
    pub not_applicable: f64,
    pub unsure: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub answer: Answer,
    pub confidence: f64,
    pub probabilities: Probabilities,
}

#[cfg(test)]
mod tests;
