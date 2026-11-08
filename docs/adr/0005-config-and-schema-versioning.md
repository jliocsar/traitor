# Config, schema and upgrades

A repo's settings live in `.agents/traitor.json`, beside its traits. Its `$schema` points at `schema/traitor.schema.json` in the traitor repo, pinned to a release tag rather than `main`, so a config never validates against a schema newer than the binary reading it. The schema is generated from the Rust config type, and CI fails when the committed file differs from the generated one.

`traitor init` both creates and upgrades: with no config it writes one, and with an older one it bumps `$schema` to the binary's version and runs any migrations. The tag in the `$schema` URL is what tells it which version it's migrating from, so there's no separate version field to keep in sync.

Exemptions live here too, per trait: a list of path globs, each with a required reason. Trait files only hold the rule; everything that tunes it for one repo (thresholds, levels, exemptions) is config. An inline marker in the code was rejected because the house `no-comments` rule forbids it, and per-snippet exemptions (accepting one snippet by its hash) are left out until a path-level one isn't enough.
