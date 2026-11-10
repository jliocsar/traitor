# Trait file format

A trait is a folder, `.agents/traits/<slug>/`, shaped like an Agent Skill:

```text
.agents/traits/railway-not-guards/
├── TRAIT.md
├── pass/   # cases that should get a pass verdict, one snippet per file
└── fail/   # cases that should get a fail verdict
```

`TRAIT.md` is Markdown with YAML frontmatter, the same shape as `SKILL.md`, so agents already read and write it well. The folder name is the slug. The frontmatter has three fields, all single words:

- `title`: the human-readable name.
- `why`: why the trait exists. It's for people, goes into the failure message, and is sent to Jev.
- `paths`: globs. A trait is never asked about a file its globs don't match.

The body is the trait's `expect`: the one statement Jev judges against, in Markdown. Nothing else in the file counts as the requirement, and exceptions are written into it. As in `SKILL.md`, the frontmatter describes the file and the body is what gets used. The whole body is sent, so it holds only the rule; the reasoning behind it goes in `why`. A trait with a blank body doesn't load.

The files in `pass/` and `fail/` are cases, also never sent: testing a trait against snippets Jev was just shown would pass every time and prove nothing. A case's file extension picks its parser.

Nothing is sent as examples. Spike 1 found that examples in the question made no measurable difference, and the case folders can't double as examples without making `traitor test` meaningless. If examples ever help on a harder trait, they can come back as their own thing; the cache key covers the question's wording, so adding them invalidates old verdicts by itself.

## Considered Options

- `expect` as a fourth frontmatter field, with the body as prose for people that's never sent. This was the first shape. A rule of a few sentences is awkward as a YAML string, and people reading `TRAIT.md` on GitHub saw the notes rather than the rule.
- crosshatch's `name`/`subject`/`requirement`/`applicability`/`purpose`: more fields than the judgment needs.
- A `kind: code | prose` field: rejected because the parser follows the file being checked, not the trait, and one trait (every number written was measured) applies to code and docs alike.
- TOML, a TS file, or TOML frontmatter: multi-line prose and diffs are ugly in TOML strings, a TS file can't be read without running code, and almost nothing supports `+++` frontmatter.
- Cases as one `.cases.md` file per trait, or as a section of `TRAIT.md` that traitor strips: a snippet copied from a real commit drops into a folder as a plain file, with no fences to escape.
