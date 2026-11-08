# Snippets climb the syntax tree

The diff only says which lines changed; it's never what Jev sees, because `+`/`-` lines carry indirection and irrelevant state, and TypeSafe's docs warn that unneeded context costs accuracy.

For each changed range, traitor takes the smallest syntax node that covers it and climbs to the nearest complete unit: the enclosing function or method, or the top-level statement when the change is outside any function. Markdown climbs to the enclosing heading section. Changes that land in the same unit are judged once. A file that won't parse falls back to a window of lines around the change.

The climb stops at the unit; it never goes further to fill the request. Spike 1 measured why: the same snippet padded with unrelated declarations went from a mean p(fail) of 0.98 on its own to about 0.8 at 2,000 tokens and 0.64 at 8,000, with one miss along the way. Merging small neighbouring statements up to a budget, and filling every request to Jev's limit, were both considered and rejected: accuracy comes before cost, requests take about 190ms and cost a fraction of a cent, and a bigger snippet also says less about where a failure is and is re-sent whole when any line of it changes.

The budget is a ceiling, not a target. A unit over it, such as a very long function, is cut lower: the climb stops at the block around the change, and the function's signature goes along so Jev knows where it is. If even the smallest node is over Jev's normal limit, the repo's config decides whether to send it with long context or fall back to the line window.
