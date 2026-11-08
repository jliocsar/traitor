# Repo layout

```text
traitor/
├── Cargo.toml, Cargo.lock      # the workspace
├── crates/
│   ├── traitor-cli/            # the `traitor` binary: arguments, output formats, wiring
│   ├── traitor-core/           # climbing, trait definitions, cache, config; defines the judge interface
│   └── traitor-ai-typesafe/    # implements the judge interface over TypeSafe's HTTP API
├── package.json                # private Bun workspace root, publish scripts only
├── packages/
│   ├── traitor/                # @jliocsar/traitor: a small launcher plus one optional dependency per platform
│   └── traitor-<os>-<arch>/    # one package per platform, holding only that binary
├── plugin/                     # the Claude plugin: the skill and the Stop hook
├── schema/traitor.schema.json  # generated from the config type
└── docs/
```

The core never depends on HTTP or the CLI, so its tests run against a fake judge, and the one outside system sits behind its own crate. The plugin ships in the same repo and release as the binary, so the hook's output format and the skill can't drift from it.

The platform packages are split by operating system and CPU, not by JavaScript runtime: they hold a compiled binary and run no JavaScript. The main package's launcher picks the right one and runs it under whichever of `npx` or `bunx` called it. A `postinstall` that copies the binary was rejected because Bun doesn't run a dependency's install scripts unless the repo trusts it. Linux builds are static musl binaries, so one package per CPU runs on any distro.

In code, a trait is `TraitDefinition`, since `trait` is a Rust keyword and a type named `Trait` reads like one.
