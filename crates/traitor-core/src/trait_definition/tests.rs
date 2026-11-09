use std::assert_matches;

use super::*;

const RAILWAY: &str = "---\ntitle: Railway, not guards\nwhy: Guards hide how a value flows.\nexpect: A computed value flows through combinators.\npaths:\n  - \"src/**/*.ts\"\n---\n\nSome prose that isn't sent.\n\n## Pass\n\n```ts\nconst label = Option.map(name, toTitle)\n```\n\n## Fail\n\n````ts\nlet label = ''\n```\nnot a close\n```\n````\n\n```ts\nlet total = 0\n```\n";

#[test]
fn reads_the_frontmatter_and_ignores_the_body() {
    let definition =
        TraitDefinition::parse("railway-not-guards", Path::new("TRAIT.md"), RAILWAY).unwrap();
    assert_eq!(definition.title, "Railway, not guards");
    assert_eq!(
        definition.expect,
        "A computed value flows through combinators."
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
