# Trait file format

A trait is a folder, `.agents/traits/<slug>/`, shaped like an Agent Skill:

```text
.agents/traits/railway-not-guards/
├── TRAIT.md
├── pass/   # cases that should get a pass verdict, one snippet per file
└── fail/   # cases that should get a fail verdict
```

`TRAIT.md` is Markdown with YAML frontmatter, the same shape as `SKILL.md`, so agents already read and write it well. The folder name is the slug. The frontmatter has four fields, all single words:

- `title`: the human-readable name.
- `why`: why the trait exists. It's for people, goes into the failure message, and is sent to Jev.
- `expect`: the one statement Jev judges against. Nothing else in the file counts as the requirement, and exceptions are written into it.
- `paths`: globs. A trait is never asked about a file its globs don't match.

The body holds pass and fail examples, sent to Jev with every question about the trait. Cases in `pass/` and `fail/` are never sent: testing a trait against examples Jev was just shown would pass every time and prove nothing. A case's file extension picks its parser.

## Considered Options

- crosshatch's `name`/`subject`/`requirement`/`applicability`/`purpose`: more fields than the judgment needs.
- A `kind: code | prose` field: rejected because the parser follows the file being checked, not the trait, and one trait (every number written was measured) applies to code and docs alike.
- TOML, a TS file, or TOML frontmatter: multi-line prose and diffs are ugly in TOML strings, a TS file can't be read without running code, and almost nothing supports `+++` frontmatter.
- Cases as one `.cases.md` file per trait, or as a section of `TRAIT.md` that traitor strips: a snippet copied from a real commit drops into a folder as a plain file, with no fences to escape.
