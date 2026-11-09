use std::cmp::Reverse;
use std::ffi::OsStr;
use std::path::Path;

use tree_sitter::{Node, Parser, Tree};

use crate::git::LineRange;

const BUDGET_TOKENS: usize = 1000;
const BYTES_PER_TOKEN: usize = 3;
const LINE_WINDOW: usize = 20;
const FUNCTION_VALUE_KINDS: &[&str] = &[
    "arrow_function",
    "function_expression",
    "function",
    "generator_function",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Language {
    TypeScript,
    Tsx,
    JavaScript,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub first_line: usize,
    pub last_line: usize,
    pub signature: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Selection {
    start_byte: usize,
    end_byte: usize,
    signature: Option<String>,
}

struct Lines<'source> {
    source: &'source str,
    starts: Vec<usize>,
}

impl Language {
    fn from_path(path: &str) -> Option<Self> {
        match Path::new(path).extension()?.to_str()? {
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "md" | "markdown" => Some(Self::Markdown),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
            Self::JavaScript => "javascript",
            Self::Markdown => "markdown",
        }
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::Markdown => tree_sitter_md::LANGUAGE.into(),
        }
    }

    fn parse(self, source: &str) -> Option<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.grammar()).ok()?;
        parser.parse(source, None)
    }

    fn is_unit(self, node: Node) -> bool {
        match self {
            Self::Markdown => node.kind() == "section",
            Self::TypeScript | Self::Tsx | Self::JavaScript => match node.kind() {
                "function_declaration" | "generator_function_declaration" | "method_definition" => {
                    true
                }
                "lexical_declaration" | "variable_declaration" => declared_function(node).is_some(),
                _ => false,
            },
        }
    }
}

pub(crate) fn language_name(path: &str) -> &str {
    Language::from_path(path)
        .map(Language::name)
        .or_else(|| Path::new(path).extension().and_then(OsStr::to_str))
        .unwrap_or("text")
}

pub(crate) fn snippets_for_changes(
    path: &str,
    source: &str,
    changes: &[LineRange],
) -> Vec<Snippet> {
    let lines = Lines::new(source);
    let parsed = Language::from_path(path)
        .and_then(|language| language.parse(source).map(|tree| (language, tree)));
    let selections = match &parsed {
        Some((language, tree)) => changes
            .iter()
            .flat_map(|change| select_for_change(*language, tree.root_node(), &lines, *change))
            .collect(),
        None => changes
            .iter()
            .filter_map(|change| lines.window(*change))
            .collect(),
    };
    outermost(selections)
        .into_iter()
        .map(|selection| lines.snippet(selection))
        .collect()
}

fn select_for_change<'lines>(
    language: Language,
    root: Node<'lines>,
    lines: &'lines Lines,
    change: LineRange,
) -> impl Iterator<Item = Selection> + 'lines {
    lines
        .content_ranges(change)
        .filter_map(move |(start, end)| {
            root.descendant_for_byte_range(start, end)
                .and_then(|covering| select_for_node(language, covering, lines.source))
                .or_else(|| lines.window(change))
        })
}

fn select_for_node(language: Language, covering: Node, source: &str) -> Option<Selection> {
    let unit = enclosing_unit(language, covering)?;
    if unit.has_error() {
        return None;
    }
    if fits(unit, source) {
        return Some(Selection::of(unit, None));
    }
    let block = largest_fitting(covering, unit, source)?;
    Some(Selection::of(block, Some(signature(unit, source))))
}

fn enclosing_unit(language: Language, covering: Node) -> Option<Node> {
    let mut node = covering;
    loop {
        if language.is_unit(node) {
            return Some(
                node.parent()
                    .filter(|parent| parent.kind() == "export_statement")
                    .unwrap_or(node),
            );
        }
        let parent = node.parent()?;
        if parent.parent().is_none() {
            return Some(node);
        }
        node = parent;
    }
}

fn largest_fitting<'tree>(
    covering: Node<'tree>,
    unit: Node<'tree>,
    source: &str,
) -> Option<Node<'tree>> {
    let mut best = None;
    let mut node = covering;
    while node != unit && fits(node, source) {
        best = Some(node);
        node = node.parent()?;
    }
    best
}

fn estimate_token_count(text: &str) -> usize {
    text.len().div_ceil(BYTES_PER_TOKEN)
}

fn fits(node: Node, source: &str) -> bool {
    estimate_token_count(&source[node.byte_range()]) <= BUDGET_TOKENS
}

fn signature(unit: Node, source: &str) -> String {
    let end = function_body(unit).map_or_else(
        || {
            source[unit.byte_range()]
                .find('\n')
                .map_or(unit.end_byte(), |offset| unit.start_byte() + offset)
        },
        |body| body.start_byte(),
    );
    source[unit.start_byte()..end].trim_end().to_string()
}

fn function_body(node: Node) -> Option<Node> {
    node.child_by_field_name("body")
        .or_else(|| match node.kind() {
            "export_statement" => node
                .child_by_field_name("declaration")
                .and_then(function_body),
            "lexical_declaration" | "variable_declaration" => {
                declared_function(node).and_then(function_body)
            }
            _ => None,
        })
}

fn declared_function(declaration: Node) -> Option<Node> {
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .filter_map(|declarator| declarator.child_by_field_name("value"))
        .find(|value| FUNCTION_VALUE_KINDS.contains(&value.kind()))
}

fn outermost(mut selections: Vec<Selection>) -> Vec<Selection> {
    selections.sort_by_key(|selection| (selection.start_byte, Reverse(selection.end_byte)));
    selections.dedup_by(|inner, outer| inner.end_byte <= outer.end_byte);
    selections
}

impl Selection {
    fn of(node: Node, signature: Option<String>) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            signature,
        }
    }
}

impl<'source> Lines<'source> {
    fn new(source: &'source str) -> Self {
        let mut starts: Vec<usize> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(index, _)| index + 1))
            .collect();
        if starts.len() > 1 && starts.last() == Some(&source.len()) {
            starts.pop();
        }
        Self { source, starts }
    }

    fn clamp(&self, change: LineRange) -> Option<(usize, usize)> {
        let last = change.last.min(self.starts.len());
        (change.first >= 1 && change.first <= last).then_some((change.first, last))
    }

    fn byte_range(&self, line: usize) -> (usize, usize) {
        let start = self.starts[line - 1];
        let end = self
            .starts
            .get(line)
            .map_or(self.source.len(), |next| next - 1);
        (start, end)
    }

    fn content_ranges(&self, change: LineRange) -> impl Iterator<Item = (usize, usize)> {
        self.clamp(change)
            .into_iter()
            .flat_map(|(first, last)| first..=last)
            .filter_map(|line| {
                let (start, end) = self.byte_range(line);
                let text = &self.source[start..end];
                let leading = text.len() - text.trim_start().len();
                let trailing = text.len() - text.trim_end().len();
                (leading < text.len()).then_some((start + leading, end - trailing))
            })
    }

    fn window(&self, change: LineRange) -> Option<Selection> {
        let (first, last) = self.clamp(change)?;
        let first = first.saturating_sub(LINE_WINDOW).max(1);
        let last = (last + LINE_WINDOW).min(self.starts.len());
        Some(Selection {
            start_byte: self.byte_range(first).0,
            end_byte: self.byte_range(last).1,
            signature: None,
        })
    }

    fn line_of(&self, byte: usize) -> usize {
        self.starts.partition_point(|start| *start <= byte)
    }

    fn snippet(&self, selection: Selection) -> Snippet {
        let line_start = self.starts[self.line_of(selection.start_byte) - 1];
        let indent = &self.source[line_start..selection.start_byte];
        let start_byte = if indent.trim().is_empty() {
            line_start
        } else {
            selection.start_byte
        };
        let text = &self.source[start_byte..selection.end_byte];
        let text = text.trim_end_matches(['\n', '\r']);
        Snippet {
            first_line: self.line_of(selection.start_byte),
            last_line: self.line_of(start_byte + text.len().saturating_sub(1)),
            signature: selection.signature,
            text: text.to_string(),
        }
    }
}

#[cfg(test)]
mod tests;
