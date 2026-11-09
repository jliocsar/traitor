---
name: traits
description: Write and test traitor traits, the house rules in a repo's .agents/traits/ that traitor checks changed code against through TypeSafe's Jev. Use when the person corrects code or prose in a way that sounds like a rule for the whole repo, when they ask to add, change or test a trait, when traitor's Stop hook blocks the turn, or when they mention traitor.
---

# Traits

A trait is a house rule no linter can check, kept as one folder in `.agents/traits/<slug>/`. traitor sends each changed snippet of code or prose to TypeSafe's Jev, with each trait whose `paths` match its file, and gets back a verdict: pass, fail, n/a (the snippet has nothing the trait is about) or unsure, each with a probability.

Run traitor the way the repo pins it: `bunx traitor` or `npx traitor` when it's in the repo's `devDependencies`, plain `traitor` otherwise. It reads `TYPESAFE_API_KEY` from the environment. `traitor check` judges everything unpushed.

## When the Stop hook blocks the turn

The reason names each failing trait, why it exists, what it expects, and where it fails. Fix the code so it holds the trait, then stop again, and the hook judges the new code.

If a verdict looks wrong, don't contort working code to please it. Tell the person which trait and which snippet, and why. The hook blocks only once on the same fails, so stopping again with them unchanged ends the turn and lists them for the person.

Snippets listed as unsure aren't failures. Read each one, and tell the person if it looks like it breaks the trait.

## Proposing a trait

When the person corrects something and the correction sounds like it applies beyond this one spot, offer a trait. Say in a sentence what it would expect and which files it covers, and ask. Write nothing until they say yes.

Read every `TRAIT.md` in `.agents/traits/` first. If one already covers the rule, offer to add a case to it instead of writing a near-copy.

A repo without `.agents/traitor.json` hasn't set traitor up. Running `traitor init` is how a repo agrees to send its code to TypeSafe, so that's the person's call: tell them, and run it only when they ask.

The person approves a trait by reviewing and committing it like any other change.

## TRAIT.md

The folder name is the slug: lowercase kebab-case, naming the rule, such as `railway-not-guards`. `TRAIT.md` is YAML frontmatter with exactly these four fields, then an optional body:

```markdown
---
title: "Railway, not guards"
why: "A ladder of guards feeding a mutable variable hides how a value flows. A chain of combinators shows the flow and short-circuits on failure."
expect: "A computed value flows through Option, Result or Effect combinators. Code breaks this when if-guards and loops reassign a let or push into a mutable accumulator to build the value. An early return that only narrows a type is fine."
paths:
  - "src/**/*.ts"
---

The reasoning, links and history, for people.
```

- `title`: the name people see.
- `why`: why the rule exists. It goes into the failure message, and Jev reads it too.
- `expect`: the one statement Jev judges against. Nothing else in the file counts. Say what code holding the trait looks like, then what breaks it, and write the exceptions in. Name what the trait is about clearly enough that code with nothing of the kind gets n/a rather than a guess.
- `paths`: globs from the repo root, where `*` stops at a slash and `**` crosses them. A trait is never asked about a file its globs don't match.

The body is never sent.

## Cases

Cases test the wording of `expect`. Each is one snippet in its own file, judged whole:

- `pass/`: snippets that hold the trait. n/a counts as holding too, since it fails nothing.
- `fail/`: snippets that break it.

Take them from the correction just made: the code before the fix is a fail case and the code after it is a pass case. Write at least one of each. Name each file for what it shows, with the extension of its language, since that picks the parser. Jev sees every case as `case.<extension>`, so neither the folder nor the name gives the answer away.

Then run `traitor test <slug>` and show the person its output. It prints each case's verdict and p(fail), then the p(fail) range over each folder. Testing sends the cases to TypeSafe, like a check does.

When a case misses, the wording of `expect` is what's wrong: reword it and test again. Never edit or move a case to make it hold. If a case looks like it's in the wrong folder, ask the person.

## Thresholds

A trait only fails a check, or blocks the Stop hook, once it has a threshold: the p(fail) at or above which a snippet fails. Without one its verdicts are only reported. Thresholds and exemptions live in `.agents/traitor.json`:

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

A threshold that separates the cases sits above the highest p(fail) on the pass cases and at or below the lowest on the fail cases. Propose one from `traitor test`'s ranges, with those numbers, and let the person pick it.
