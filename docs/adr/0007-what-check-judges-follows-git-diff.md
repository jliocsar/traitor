# What `check` judges follows `git diff`

`traitor check` takes an optional ref with `git diff`'s meaning, so nobody learns new rules:

- No ref: everything unpushed, including uncommitted and untracked files. The base is `@{upstream}`; a branch never pushed falls back to its merge-base with `origin/HEAD`, and a repo with no remote to `HEAD`.
- A ref (`HEAD~`, a sha): everything from that base to the working tree.
- A range (`A..B`): only what changed between two commits, with snippets read from `B`'s files rather than the working tree.

The default matters because agents commit mid-turn: a diff against `HEAD` would be empty after a turn that committed twice. traitor doesn't know about turns or hooks. Whoever wires the Stop hook, pre-push or CI picks the ref; CI typically passes the PR's base branch.

Re-running over the same unpushed work is expected to be cheap because verdicts are cached by trait, snippet and model, so only changed snippets reach Jev.
