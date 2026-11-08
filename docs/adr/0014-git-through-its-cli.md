# Git through its CLI, not gitoxide

traitor runs the `git` binary for everything it needs from git: the upstream, merge-bases, the diff, untracked files and file contents at a commit. ADR 0007 promises that `check`'s argument means exactly what it means to `git diff`, and asking git itself is the surest way to keep that promise.

gitoxide (`gix`) was considered because it's pure Rust and spawns no processes. Speed was no reason to pick it: in pie the calls a check needs took 1.5-11ms each, about 20ms together, against roughly 190ms for each Jev request. It would add a large dependency tree and make traitor rely on its own reading of `@{upstream}`, ranges and merge-bases matching git's. `git2` was rejected for its C bindings, which make cross-compiling harder.
