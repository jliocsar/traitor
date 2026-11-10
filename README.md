<h1 align="center">
  <img src=".github/assets/traitor.svg" alt="traitor" width="100%">
</h1>

Every repo has rules no linter can check. "Errors say what to do next." "Docs use the present tense." "Build values with combinators, not a pile of if-guards." They live in reviewers' heads, and they slip.

traitor checks your changed code and docs against those rules. It sends each changed piece, with the rules that apply to it, to [TypeSafe](https://typesafe.ai)'s Jev, and gets back a verdict for each: pass, fail, n/a or unsure, with how likely each one is.

A rule is called a **trait**. It's a folder in `.agents/traits/` with a `TRAIT.md` that spells the rule out, plus a few example snippets that hold it and break it.

## Install

Pin it in the repo, so the agent, the Stop hook and CI all run the same traitor:

```sh
bun add --dev @jliocsar/traitor
# or
npm install --save-dev @jliocsar/traitor
```

It's a dev dependency, so run it through your package manager: `npx traitor`, `bunx traitor` or `pnpm exec traitor`. The examples below just say `traitor`.

For a one-off run outside a repo that has it, use `npx @jliocsar/traitor`. Plain `npx traitor` there would fetch a different npm package that happens to be called `traitor`.

No `package.json`? Each GitHub release also has the binaries.

## Usage

### 1. Set it up

```sh
export TYPESAFE_API_KEY=...
traitor init
```

This writes `.agents/traitor.json`. Running `init` is how a repo agrees to send its code to TypeSafe, since every `check` and `test` sends snippets there. Commit the file so everyone on the repo can see that choice.

### 2. Write a trait

Say your team keeps asking for combinators instead of if-guards. Make a folder for the rule:

```
.agents/traits/railway-not-guards/
├── TRAIT.md
├── pass/
│   └── combinators.ts
└── fail/
    └── guard-ladder.ts
```

`TRAIT.md` says what the rule is:

```markdown
---
title: "Railway, not guards"
why: "A ladder of guards feeding a mutable variable hides how a value flows."
paths:
  - "src/**/*.ts"
---

A computed value flows through Option, Result or Effect combinators.
Code breaks this when if-guards and loops reassign a `let` to build the value.
An early return that only narrows a type is fine.
```

The frontmatter on top:

- **title**: the name people see.
- **why**: why the rule exists. It shows up in every failure.
- **paths**: which files the rule covers. `*` stays inside a folder and `**` goes through them.

The body is the rule itself, and it's exactly what Jev judges against. Say what good code looks like, what breaks the rule, and any exceptions. Keep it to the rule: the whole body is sent, so notes and links go in `why` instead.

The `pass/` and `fail/` folders hold small examples, one snippet per file. They test the rule's wording.

### 3. Test the wording

```sh
traitor test railway-not-guards
```

```
ok: .agents/traits/railway-not-guards/fail/guard-ladder.ts expects fail, got fail, p(fail) 0.93
ok: .agents/traits/railway-not-guards/pass/combinators.ts expects pass, got pass, p(fail) 0.04

railway-not-guards: p(fail) on fail cases 0.93 to 0.93, on pass cases 0.04 to 0.04

2 of 2 cases got the verdict their folder names, 0 from the cache.
```

A `MISS` means the trait's wording is off. Reword the body and test again. Don't move the example to make it pass.

Run `traitor test` with no name to test every trait.

### 4. Check your changes

```sh
traitor check               # everything you haven't pushed yet
traitor check main          # everything since main
traitor check HEAD~3..HEAD  # what changed between two commits
```

```
fail: Railway, not guards (railway-not-guards) at src/config.ts:12-31, p(fail) 0.91
  why: A ladder of guards feeding a mutable variable hides how a value flows.

traitor judged 14 snippets: 1 fail, 0 unsure, 9 verdicts from the cache.
```

traitor only sends the changed bits: the function, block or doc section around each change, never the whole repo. Verdicts are cached, so checking again is quick.

An **unsure** isn't a failure. It means Jev couldn't tell from the snippet alone, so take a look yourself.

### 5. Make it block

Out of the box, traitor only reports. To make a trait fail the check, give it a threshold in `.agents/traitor.json`:

```json
{
  "traits": {
    "railway-not-guards": {
      "threshold": 0.5,
      "exempt": [{ "paths": ["src/generated/**"], "reason": "Generated code." }]
    }
  }
}
```

Now `check` exits with `1` when a snippet's p(fail) reaches `0.5`. Pick a threshold between the numbers `traitor test` printed: above the pass cases, at or below the fail cases.

`exempt` skips paths the rule shouldn't judge, each group with a reason.

| Exit code | Meaning |
|---|---|
| `0` | All good, or the fails were only reported |
| `1` | A trait with a threshold failed |
| `2` | traitor couldn't run the check |

### In CI

On GitHub Actions, failures show up as annotations right on the changed lines:

```yaml
- uses: actions/checkout@v7
  with:
    fetch-depth: 0
- run: npm ci
- run: npx traitor check origin/main
  env:
    TYPESAFE_API_KEY: ${{ secrets.TYPESAFE_API_KEY }}
```

## Claude Code

```sh
claude plugin marketplace add jliocsar/traitor
claude plugin install traitor@traitor
```

The plugin adds two things:

- **A `traits` skill.** When you correct Claude in a way that sounds like a house rule, it offers to write a trait for it. It writes the trait once you agree, then tests it.
- **A Stop hook.** When Claude finishes a turn, the hook runs `traitor check`. If a trait over its threshold fails, Claude goes back and fixes it.

## Docs

- `CONTEXT.md` is the glossary.
- `docs/adr/` holds the big decisions.
- `docs/v1-scope.md` holds the smaller ones.
