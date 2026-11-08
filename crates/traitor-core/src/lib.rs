#[cfg(test)]
mod grammar_compatibility {
    use tree_sitter::{Language, Parser};

    fn parses_without_errors(language: Language, source: &str) {
        let mut parser = Parser::new();
        parser.set_language(&language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        assert!(
            !tree.root_node().has_error(),
            "{}",
            tree.root_node().to_sexp()
        );
    }

    #[test]
    fn typescript() {
        parses_without_errors(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "export const add = (left: number, right: number): number => left + right\n",
        );
    }

    #[test]
    fn tsx() {
        parses_without_errors(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "export const View = (props: { name: string }) => <div>{props.name}</div>\n",
        );
    }

    #[test]
    fn javascript() {
        parses_without_errors(
            tree_sitter_javascript::LANGUAGE.into(),
            "export function add(left, right) { return left + right }\n",
        );
    }

    #[test]
    fn markdown() {
        parses_without_errors(
            tree_sitter_md::LANGUAGE.into(),
            "# Title\n\nSome text.\n\n## Section\n\n- item\n",
        );
    }
}
