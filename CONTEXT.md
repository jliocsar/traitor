# traitor

Checks changed code and docs against a repo's house rules that no linter can check, by asking TypeSafe's Jev whether each changed piece holds each rule.

## Language

**Trait**:
A written house rule that a piece of code or prose either holds or breaks, kept as one folder in `.agents/traits/`.
_Avoid_: rule, spec, contract, preference

**Snippet**:
The piece of a changed file that gets judged: the function, top-level statement or heading section that encloses a change.
_Avoid_: chunk, hunk, diff

**Budget**:
The estimated token size above which a snippet is cut down to the block around the change.
_Avoid_: limit, context window

**Long context**:
Jev's opt-in mode with larger request limits, used only when even the smallest snippet around a change doesn't fit the normal ones.

**Verdict**:
Jev's answer for one trait and one snippet: pass, fail, n/a (the snippet has nothing the trait is about) or unsure.
_Avoid_: result, assessment, finding

**Example**:
A short pass or fail illustration written in a trait, sent to Jev with every question about that trait.

**Case**:
A snippet stored beside a trait with the verdict it should get, used only to test the trait and never sent as an example.
_Avoid_: fixture, test
