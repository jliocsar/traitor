# Shipped on npm, one package per platform

traitor ships the way oxlint and biome do: `@jliocsar/traitor` depends on one optional package per platform, each holding only that platform's binary. A repo pins the version in its `devDependencies`, so the agent, the Stop hook and CI run the same traitor. That matters because traits and config are versioned per repo: a config upgraded by a newer `traitor init` must not reach a teammate whose machine-wide traitor is older. The same binaries are attached to each GitHub release, so mise can install them where there's no `package.json`.

There's no `traitor update`. The package manager updates traitor, and a self-update would overwrite a binary it manages.

## Considered Options

- mise from GitHub releases as the main channel, as pie does: one version per machine, so two repos can't pin different ones.
- An install script with a self-updating binary in `~/.local/bin`: same per-machine problem, plus a second updater.
