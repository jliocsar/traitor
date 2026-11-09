use super::*;

const MODULE: &str = r#"import * as Option from 'effect/Option'

const LIMIT = 10

export function label(name: string | undefined): string {
  let result = 'untitled'
  if (name !== undefined) {
    result = name.toUpperCase()
  }
  return result
}

export class Greeter {
  private readonly greeting = 'hello'

  greet(name: string): string {
    return `${this.greeting} ${name}`
  }

  wave(): string {
    return 'wave'
  }
}

export const toTitle = (name: string) =>
  Option.liftPredicate(name, (value) => value.length > 0).pipe(
    Option.map((value) => value.toUpperCase()),
  )
"#;

const GUIDE: &str = "# Guide\n\nIntro.\n\n## First\n\nPara one.\n\n## Second\n\nPara two.\n";

fn lines(first: usize, last: usize) -> Change {
    Change::Added(LineRange { first, last })
}

fn render(snippets: &[Snippet]) -> String {
    snippets
        .iter()
        .map(|snippet| {
            let signature = snippet
                .signature
                .as_ref()
                .map(|signature| format!(" inside `{signature}`"))
                .unwrap_or_default();
            format!(
                "lines {}-{}{signature}\n{}",
                snippet.first_line, snippet.last_line, snippet.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n-----\n")
}

fn long_function() -> String {
    let filler: String = (0..400)
        .map(|index| format!("  const value{index} = {index}\n"))
        .collect();
    format!(
        "function long(input: number): number {{\n{filler}  if (input > 0) {{\n    let total = input\n    total += 1\n  }}\n{filler}  return input\n}}\n"
    )
}

#[test]
fn grammars_load() {
    for (path, source) in [
        (
            "a.ts",
            "const add = (left: number, right: number): number => left + right\n",
        ),
        (
            "a.tsx",
            "const View = (props: { name: string }) => <div>{props.name}</div>\n",
        ),
        (
            "a.js",
            "export function add(left, right) { return left + right }\n",
        ),
        ("a.md", "# Title\n\nSome text.\n"),
    ] {
        let language = Language::from_path(path).unwrap();
        let tree = language.parse(source).unwrap();
        assert!(!tree.root_node().has_error(), "{path}");
    }
}

#[test]
fn a_change_inside_a_function_climbs_to_the_function() {
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/module.ts",
        MODULE,
        &[lines(8, 8)]
    )));
}

#[test]
fn a_change_outside_any_function_climbs_to_the_top_level_statement() {
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/module.ts",
        MODULE,
        &[lines(3, 3)]
    )));
}

#[test]
fn changes_in_two_methods_are_two_snippets() {
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/module.ts",
        MODULE,
        &[lines(17, 17), lines(21, 21)]
    )));
}

#[test]
fn a_callback_climbs_past_itself_to_the_named_function() {
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/module.ts",
        MODULE,
        &[lines(26, 26)]
    )));
}

#[test]
fn blank_lines_select_nothing() {
    assert_eq!(
        snippets_for_changes("src/module.ts", MODULE, &[lines(4, 4)]),
        Vec::new()
    );
}

#[test]
fn a_whole_new_file_is_every_top_level_unit() {
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/module.ts",
        MODULE,
        &[Change::WHOLE_FILE]
    )));
}

#[test]
fn a_function_over_the_budget_is_cut_to_the_block_around_the_change() {
    let source = long_function();
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/long.ts",
        &source,
        &[lines(404, 404)]
    )));
}

#[test]
fn markdown_climbs_to_the_heading_section() {
    insta::assert_snapshot!(render(&snippets_for_changes(
        "AGENTS.md",
        GUIDE,
        &[lines(7, 7)]
    )));
}

#[test]
fn a_unit_that_does_not_parse_falls_back_to_a_line_window() {
    let source = "function broken( {\n  return 1\n}\n";
    insta::assert_snapshot!(render(&snippets_for_changes(
        "src/broken.ts",
        source,
        &[lines(2, 2)]
    )));
}

#[test]
fn a_broken_unit_in_a_new_file_windows_only_its_own_lines() {
    let broken = "function broken( {\n  return 1\n}\n";
    let source = format!("{MODULE}\n{broken}\n{MODULE}");
    let snippets = snippets_for_changes("src/module.ts", &source, &[Change::WHOLE_FILE]);
    assert!(snippets.len() > 3);
    assert!(
        snippets
            .iter()
            .all(|snippet| estimate_token_count(&snippet.text) <= BUDGET_TOKENS)
    );
}

#[test]
fn a_long_new_file_in_an_unknown_language_is_cut_to_the_budget() {
    let source: String = (1..=2000).map(|line| format!("line {line}\n")).collect();
    let snippets = snippets_for_changes("src/main.rs", &source, &[Change::WHOLE_FILE]);
    assert!(snippets.len() > 1);
    assert!(
        snippets
            .iter()
            .all(|snippet| estimate_token_count(&snippet.text) <= BUDGET_TOKENS)
    );
    assert_eq!(
        snippets
            .iter()
            .map(|snippet| snippet.text.lines().count())
            .sum::<usize>(),
        2000
    );
}

#[test]
fn an_empty_new_file_selects_nothing() {
    assert_eq!(
        snippets_for_changes("src/__init__.py", "", &[Change::WHOLE_FILE]),
        Vec::new()
    );
}

#[test]
fn blank_lines_in_an_unknown_language_select_nothing() {
    let source = "a = 1\n\n\nb = 2\n";
    assert_eq!(
        snippets_for_changes("src/main.py", source, &[lines(2, 3)]),
        Vec::new()
    );
}

#[test]
fn an_unknown_language_falls_back_to_a_line_window() {
    let source: String = (1..=50).map(|line| format!("line {line}\n")).collect();
    let snippets = snippets_for_changes("src/main.rs", &source, &[lines(30, 31)]);
    assert_eq!(
        snippets
            .iter()
            .map(|snippet| (snippet.first_line, snippet.last_line))
            .collect::<Vec<_>>(),
        vec![(10, 50)]
    );
}

#[test]
fn leading_context_gives_way_to_the_budget() {
    let context = format!("{}\n", "x".repeat(200)).repeat(20);
    let source = format!("{context}changed\n");
    let snippets = snippets_for_changes("src/x.py", &source, &[lines(21, 21)]);
    assert!(
        snippets
            .iter()
            .all(|snippet| estimate_token_count(&snippet.text) <= BUDGET_TOKENS)
    );
    assert_eq!(snippets.last().map(|snippet| snippet.last_line), Some(21));
}

#[test]
fn a_window_stops_at_a_unit_that_was_placed() {
    let broken = "function broken( {\n  return 1\n}\n";
    let source = format!("{MODULE}\n{broken}\n{MODULE}");
    let snippets = snippets_for_changes("src/module.ts", &source, &[Change::WHOLE_FILE]);
    assert!(
        snippets
            .windows(2)
            .all(|pair| pair[0].last_line < pair[1].first_line)
    );
}

#[test]
fn a_deletion_between_blank_lines_reaches_the_nearest_code_on_each_side() {
    let source = "def a():\n    pass\n\n\n\n\ndef b():\n    pass\n";
    let snippets = snippets_for_changes("m.py", source, &[Change::Deleted { after: 4 }]);
    assert_eq!(
        snippets
            .iter()
            .map(|snippet| (snippet.first_line, snippet.last_line))
            .collect::<Vec<_>>(),
        vec![(1, 8)]
    );
}

#[test]
fn a_window_on_a_units_last_line_does_not_repeat_the_unit() {
    let body: String = (1..=25).map(|index| format!("  x{index}()\n")).collect();
    let source = format!("function a() {{\n{body}}}; foo()\n");
    let snippets = snippets_for_changes("src/a.ts", &source, &[lines(2, 2), lines(27, 27)]);
    assert!(
        snippets
            .windows(2)
            .all(|pair| pair[0].last_line <= pair[1].first_line)
    );
}
