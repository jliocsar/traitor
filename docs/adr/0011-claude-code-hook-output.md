# Claude Code hook output comes from the binary

Claude Code reads a Stop hook's exit code 2 as "block and feed stderr to the agent", while traitor's exit 2 means it couldn't check. Wiring `traitor check` in directly would trap an offline agent in its turn, the opposite of ADR 0009.

So `traitor check --format claude-code-hook` speaks the hook's language itself: it prints the JSON a Stop hook returns and exits 0. A fail becomes `{"decision": "block", "reason": …}` with the trait and the snippet; unsure snippets go in the reason as ones to look at; couldn't-check lets the agent stop and shows a `systemMessage`. A wrapper script in the plugin mapping exit codes was rejected: the mapping belongs in the binary, where it's tested, and the plugin's hook stays one line.

A hook that blocks keeps the agent going, and an agent that explains a fail instead of fixing it leaves the code, and so the cached verdict, unchanged. Claude Code only ends that loop after eight blocks in a row. So the hook blocks once per verdict: traitor records the fails it blocked on under the hook input's `session_id`, beside the cache, and when the agent stops again with exactly those fails it lets the agent stop and lists what's still failing in a `systemMessage`. A new or changed fail blocks again. Never blocking twice in a row (whenever `stop_hook_active` is true) was rejected because a fail introduced while fixing another would never block. Exit 1 was rejected because Claude Code treats it as a non-blocking error the agent never sees.

The plugin ships the skill and a Stop hook running plain `traitor check --format claude-code-hook`, which does nothing in a repo without `.agents/traits/`. A repo that wants another ref or other behaviour wires its own hook instead.
