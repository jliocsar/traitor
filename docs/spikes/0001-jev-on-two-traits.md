# Spike 1: Jev on two traits

Run 2026-11-08 against `jev-latest`, which answered as `jev-1.14.0`. The scripts and raw results are in `tmp/spike/` (gitignored).

## Setup

Two traits, `railway-not-guards` (TypeScript) and `agents-md-names-no-file` (Markdown). Each request was one Choice with the four verdicts, the snippet as state `{ path, language, snippet }`, and the trait as structured instructions `{ question, trait: { title, why, expect, examples? } }`.

Cases are real code and prose from public repos, labelled by hand:

- railway: 7 fail cases (guard ladders feeding a `let` or a mutable accumulator, from crosshatch/traits and begone-slop) and 9 pass cases (Option/Effect chains from pie, type-narrowing guards from begone-slop, one schema with no computation at all).
- AGENTS.md: 5 fail cases and 5 pass cases, paragraphs from pie's AGENTS.md.

Each railway case ran at three sizes: the snippet alone, and the snippet in the middle of padding up to about 2,000 and 8,000 estimated tokens. The padding was Effect's `Array.d.ts` with comments stripped: declarations only, nothing the trait is about. Every case ran with and without the trait's examples. 116 requests in all, then the 52 unpadded ones again to check stability.

## Results

| railway, snippet size | caught | false alarms | mean p(fail) on fail cases | mean p(fail) on pass cases |
|---|---|---|---|---|
| alone | 7/7 and 7/7 | 0/9 and 0/9 | 0.98 / 0.99 | 0.00 / 0.01 |
| ~2,000 tokens | 6/7 and 7/7 | 0/9 and 0/9 | 0.77 / 0.84 | 0.03 / 0.05 |
| ~8,000 tokens | 7/7 and 7/7 | 0/9 and 0/9 | 0.64 / 0.69 | 0.10 / 0.12 |

Each cell is with examples / without examples.

AGENTS.md, paragraph alone: 5/5 caught and 0/5 false alarms both ways, mean p(fail) 0.97-0.98 on fail cases and 0.04-0.06 on pass cases.

- **Size costs accuracy.** On its own, a snippet separates almost perfectly. Padded with unrelated declarations, the margin shrinks steadily, and the one miss (a mutable `Set` filled in a loop, 0.40 fail against 0.47 pass) came at 2,000 tokens. Real neighbouring code would likely hurt more than inert declarations.
- **Examples didn't help** on these cases: no case flipped because of them, and at 2,000 tokens the run with examples did slightly worse. Too few cases to call it either way.
- **n/a works.** The schema with no computation came back n/a every time, at 0.92-0.97 when alone; padding pushed it toward pass.
- **Stable.** Re-running the 52 unpadded requests moved p(fail) by 0.004 on average, 0.04 at most.
- **Fast and cheap.** Median latency 185-195ms alone and about 240ms at 8,000 tokens. All 168 requests cost $0.03.
- **bytes / 4 underestimates.** Measured input tokens ran about 1.35x the bytes / 4 estimate of the request body at every size, closer to bytes / 3 for code.

## Limits

26 cases, labelled by one person, from three repos. Two of the original AGENTS.md pass cases were dropped before the run because the trait's wording made them ambiguous: they named a config key and a field in passing. That ambiguity is a finding about writing `expect`, not about Jev.
