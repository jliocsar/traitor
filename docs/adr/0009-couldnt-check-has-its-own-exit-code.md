# "Couldn't check" has its own exit code

`check` exits 0 when every judged trait holds or doesn't apply, 1 when a trait breaks, and 2 when it couldn't check at all: no `TYPESAFE_API_KEY`, offline, or TypeSafe answering with errors. The message names the cause.

Passing silently would break the rule that silence isn't a pass, and failing with 1 would look like a real trait failure and leave an offline agent looping on a Stop hook it can't satisfy. As with the ref `check` takes, traitor reports what happened and the hook or CI decides: a Stop hook can let the agent stop, and CI, which should always have the key, can fail.
