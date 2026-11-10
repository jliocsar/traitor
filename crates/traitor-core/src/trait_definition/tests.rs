use std::assert_matches;

use super::*;

const RAILWAY: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\npaths:\n  - \"src/**/*.ts\"\n---\n\nA computed value flows through combinators.\n\nAn early return that only narrows a type is fine.\n";

#[test]
fn the_body_is_what_the_trait_expects() {
    let definition =
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), RAILWAY).unwrap();
    assert_eq!(definition.title, "Railway, not guards");
    assert_eq!(
        definition.expect,
        "A computed value flows through combinators.\n\nAn early return that only narrows a type is fine."
    );
}

#[test]
fn a_blank_body_is_missing_what_the_trait_expects() {
    let contents = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\npaths: [\"src/**/*.ts\"]\n---\n\n  \n";
    assert_matches!(
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), contents),
        Err(TraitDefinitionError::MissingExpect { .. })
    );
}

#[test]
fn expect_in_the_frontmatter_is_an_unknown_field() {
    let contents = RAILWAY.replace("paths:", "expect: Combinators.\npaths:");
    assert_matches!(
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), &contents),
        Err(TraitDefinitionError::InvalidFrontmatter { .. })
    );
}

#[test]
fn paths_match_with_literal_separators() {
    let definition =
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), RAILWAY).unwrap();
    assert!(definition.applies_to("src/module.ts"));
    assert!(definition.applies_to("src/deep/module.ts"));
    assert!(!definition.applies_to("test/module.ts"));
    assert!(!definition.applies_to("src/module.tsx"));
}

#[test]
fn rejects_unknown_frontmatter_fields() {
    let contents = RAILWAY.replace("paths:", "kind: code\npaths:");
    assert_matches!(
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), &contents),
        Err(TraitDefinitionError::InvalidFrontmatter { .. })
    );
}

#[test]
fn only_a_line_of_exactly_three_dashes_closes_the_frontmatter() {
    let contents = RAILWAY.replacen("\n---\n\n", "\n----\n\n", 1);
    assert_matches!(
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), &contents),
        Err(TraitDefinitionError::MissingFrontmatter { .. })
    );
}

#[test]
fn empty_frontmatter_is_invalid_rather_than_missing() {
    assert_matches!(
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), "---\n---\n"),
        Err(TraitDefinitionError::InvalidFrontmatter { .. })
    );
}

#[test]
fn trailing_spaces_after_the_closing_dashes_are_fine() {
    let contents = RAILWAY.replacen("\n---\n\n", "\n--- \n\n", 1);
    assert!(TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), &contents).is_ok());
}
