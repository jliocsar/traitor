# v1 scope

Smaller decisions from planning that are easy to reverse, so they don't get ADRs.

- **Languages:** TypeScript, TSX and JavaScript, plus Markdown. Anything else falls back to a window of lines around the change.
- **Token estimate:** one function, `estimate_token_count`, so it's easy to change. Spike 1 measured `bytes / 4` running about 35% under Jev's real count; `bytes / 3` is closer for code.
- **Thresholds and levels:** every config field is optional and falls back to a built-in default. Until the defaults are measured, traitor reports but never fails.
- **Exemptions:** path-level only, in the config, each with a reason.
- **Stack:** clap, reqwest (with rustls, so there's no OpenSSL to cross-compile) on tokio with a concurrency cap, serde and serde_json, serde-saphyr for frontmatter, tree-sitter with its TypeScript, JavaScript and Markdown grammars, globset, sha2, dirs, schemars, snafu, insta, and the git CLI. Picked for maintenance first: speed barely matters anywhere but the network. Rejected: serde_yaml_ng (last release May 2024, built on machine-translated libyaml), zlob (a fast filesystem walker, and traitor never walks the filesystem; its crate needs libclang to build), typify (it goes from schema to Rust types, the opposite direction), rayon (a CPU tool for work that waits on the network).
- **Full-repo run:** no command of its own; `traitor check 4b825dc642cb6eb9a060e54bf8d69288fbee4904` diffs against git's empty tree.
