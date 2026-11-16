# `check` has a JSON report, and can judge whole files

Tools want traitor's verdicts too: a linter plugin shows them as diagnostics, a bot comments with them. They shouldn't parse text meant for people, and traitor shouldn't learn their formats. So `traitor check --format json` prints one JSON object, documented as a versioned contract in `docs/check-json.md`, with a schema a test keeps current. traitor stays language-agnostic: it knows nothing about oxlint or any other tool, and each tool's plugin maps the contract to its own diagnostics.

A linter checks the files it's given, not a diff (ADR 0007). So `--files` judges the files named, whole, as they are on disk. Each is cut into snippets exactly as a new untracked file is, so the cache, the budget and the findings work the same. A path outside the repository is an error rather than a skip, because no trait could ever apply to it and a silent skip would read as a pass.

The contract carries its own `version`. New fields don't bump it, and readers ignore what they don't know. An exporter that writes traits as another tool's rules was rejected: the traits would then live in two places, and traitor would have to know every tool.
