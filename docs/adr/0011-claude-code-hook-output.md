# Claude Code hook output comes from the binary

Claude Code reads a Stop hook's exit code 2 as "block, and feed stderr to the agent", but traitor's own exit 2 means it couldn't check. Wiring `traitor check` in directly would trap an offline agent in its turn, the opposite of ADR 0009.

So `traitor check --format claude-code-hook` speaks the hook's language itself. A wrapper script in the plugin mapping exit codes was rejected: the mapping belongs in the binary, where it's tested, and the plugin's hook stays one line.

- **A fail over its threshold blocks.** The hook exits 2 with the reason on stderr, which is what Claude Code hands the agent from a Stop hook. The reason names each failing trait once, with its `why` and `expect`, then where it fails. Unsure snippets go in the reason as ones to read.
- **Anything else lets the agent stop.** That covers passes, fails without a threshold, and a repo without `.agents/traits/`. The hook exits 0 and prints JSON on stdout: `{}`, or a `systemMessage` for the user.
- **Couldn't check is a note, not a block.** No key, offline, or bad hook input all exit 0 with a `systemMessage` saying why. A hook that can't even start, like arguments that don't parse, exits 1, which Claude Code shows as a non-blocking error. In hook mode traitor never exits 2 for its own trouble, or every stop would block on something the agent can't fix.

Until #7 a block printed `{"decision": "block", "reason": …}` and exited 0. Exit 2 with stderr is the channel Claude Code documents for a Stop hook's reason, so it replaced the JSON form.

A hook that blocks keeps the agent going. An agent that explains a fail instead of fixing it leaves the code, and so the cached verdict, unchanged, and Claude Code only ends that loop after eight blocks in a row. So the hook blocks once per verdict. traitor records the fails it blocked on under the hook input's `session_id`, beside the cache. When the agent stops again with exactly those fails, it lets the agent stop and lists what's still failing in a `systemMessage`. A new or changed fail blocks again.

Two alternatives were rejected:

- **Never blocking twice in a row** (whenever `stop_hook_active` is true): a fail introduced while fixing another would never block.
- **Exit 1 for a fail:** Claude Code treats it as a non-blocking error the agent never sees.

If traitor can't record the fails it's about to block on (the cache directory can't be written), it doesn't block. It would block every stop on the same fails forever. It lets the agent stop and says in a `systemMessage` which fails it couldn't remember, and why.

The plugin ships the skill and a Stop hook running plain `traitor check --format claude-code-hook`, which does nothing in a repo without `.agents/traits/`. The hook has a 300-second `timeout`, which Claude Code reads in seconds, so a hung request can't hold a stop for the 10-minute default. The hook runs the repo's own traitor from `node_modules/.bin` when there is one, so it's the version the repo pins (ADR 0006), and `traitor` on `PATH` otherwise. With neither, it does nothing, so the plugin can stay installed for every repo. A repo that wants another ref or other behaviour wires its own hook instead.
