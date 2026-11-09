# Four-answer verdict

Each trait and snippet pair is one Jev Choice with four answers: pass, fail, n/a (the snippet has nothing the trait is about) and unsure. Each comes back with a probability, and the answer comes with a confidence. The names match the trait's `pass/`, `fail/` and `n-a/` case folders, so a case's folder is the verdict it expects.

A yes/no Noul was the cheaper option, but it has no way to say the snippet contains nothing the trait is about. That's expected to be the most common case, since `paths` only filters by file. A Noul would turn those into fake passes or fake fails, and turn honest uncertainty into a guess.

Code decides what each answer does. fail over the threshold fails the check. unsure never fails it; it's reported with the snippet and the trait, and the message tells whoever ran traitor to look at it themselves, so it never passes silently. In CI it's printed as a GitHub warning annotation. pass and n/a print nothing.
