//! `traitor check --format json`: the report as a stable contract for tools.
//!
//! `docs/check-json.md` documents it, and `schema/check-report.schema.json` is its schema,
//! kept current by a test.

use serde::Serialize;
use traitor_core::check::{Finding, Report};
use traitor_core::verdict::Answer;

/// The contract's version. A change that could break a reader bumps it; a new field doesn't.
pub(crate) const VERSION: u32 = 1;

/// Everything `check` found, for a tool to read.
#[derive(Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) struct JsonReport<'report> {
    /// The contract's version, 1 for this shape.
    version: u32,
    /// The repository's root, absolute. Each finding's path is relative to it.
    root: &'report str,
    /// How many snippets were judged.
    snippet_count: usize,
    /// Whether a finding failed its trait's threshold, which is when `check` exits 1.
    fails_check: bool,
    /// One per snippet and trait that applies to it, passes included.
    findings: Vec<JsonFinding<'report>>,
    /// Paths that aren't valid UTF-8, shown as near as they can be. They weren't judged.
    non_utf8_paths: &'report [String],
}

/// One trait's verdict on one snippet.
#[derive(Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
struct JsonFinding<'report> {
    /// The file, relative to the root, with `/` between its parts.
    path: &'report str,
    /// The snippet's first line, counting from 1.
    first_line: usize,
    /// The snippet's last line, included.
    last_line: usize,
    /// The trait's slug: its folder's name under `.agents/traits/`.
    #[serde(rename = "trait")]
    trait_slug: &'report str,
    /// The trait's title, from its `TRAIT.md`.
    title: &'report str,
    /// Why the trait matters, from its `TRAIT.md`.
    why: &'report str,
    /// The answer Jev gave.
    verdict: JsonVerdict,
    /// How likely the snippet fails the trait, from 0 to 1.
    p_fail: f64,
    /// The trait's threshold in `.agents/traitor.json`, or null when its fails are only reported.
    threshold: Option<f64>,
    /// Whether `pFail` reached the threshold.
    fails_check: bool,
    /// Whether the verdict came from traitor's cache instead of a new request.
    from_cache: bool,
}

/// Jev's answer, named as Jev names it.
#[derive(Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
enum JsonVerdict {
    #[serde(rename = "pass")]
    Pass,
    #[serde(rename = "fail")]
    Fail,
    #[serde(rename = "n/a")]
    NotApplicable,
    #[serde(rename = "unsure")]
    Unsure,
}

impl<'report> JsonReport<'report> {
    /// `report` for a tool, with `root` as the repository its paths are relative to.
    pub(crate) fn new(report: &'report Report<'_>, root: &'report str) -> Self {
        Self {
            version: VERSION,
            root,
            snippet_count: report.snippet_count,
            fails_check: report.fails_check(),
            findings: report.findings.iter().map(JsonFinding::from).collect(),
            non_utf8_paths: &report.non_utf8_paths,
        }
    }
}

impl<'report> From<&'report Finding<'_>> for JsonFinding<'report> {
    fn from(finding: &'report Finding<'_>) -> Self {
        Self {
            path: &finding.path,
            first_line: finding.snippet.first_line,
            last_line: finding.snippet.last_line,
            trait_slug: &finding.definition.slug,
            title: &finding.definition.title,
            why: &finding.definition.why,
            verdict: finding.verdict.answer.into(),
            p_fail: finding.verdict.probabilities.fail,
            threshold: finding.threshold,
            fails_check: finding.fails_check(),
            from_cache: finding.from_cache,
        }
    }
}

impl From<Answer> for JsonVerdict {
    fn from(answer: Answer) -> Self {
        match answer {
            Answer::Pass => Self::Pass,
            Answer::Fail => Self::Fail,
            Answer::NotApplicable => Self::NotApplicable,
            Answer::Unsure => Self::Unsure,
        }
    }
}

#[cfg(test)]
mod tests;
