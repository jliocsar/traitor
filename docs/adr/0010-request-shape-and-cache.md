# Request shape and verdict cache

One Jev request per snippet. The snippet is the request's state, sent once: `{ path, language, signature, snippet }`. Every trait whose `paths` match becomes one question in that request, carrying only what's specific to it: `{ title, why, expect }`, plus the meaning of each of the four answers. Putting a trait's wording in the state instead would make every question pay for every other trait's.

Verdicts are cached on disk, one file per verdict, at `<platform cache dir>/traitor/v1/<sha256 of model, question and state>.json` (`~/.cache` on Linux, `~/Library/Caches` on macOS), written to a temp file and renamed into place. A process-lifetime map was rejected because every `traitor check` is a new process. Files rather than a database, so concurrent runs don't conflict; the user's cache dir rather than the repo, so worktrees and repos share verdicts; and the key hashes the exact question, so editing a trait's wording misses the cache on its own.
