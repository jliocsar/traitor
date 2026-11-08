# Running init is consent to send code

Every check sends snippets to TypeSafe's servers. traitor sends nothing from a repo without `.agents/traitor.json`: `traitor init` writes it, and says plainly that code will be sent to TypeSafe. Committing the file makes that choice visible to everyone on the repo. Without the file, `check` exits 2 with a message pointing at `init`.

Refusing to run against a list of git remotes (a work org, say) was rejected: that's a policy for one machine, not for traitor, and easy to get wrong.
