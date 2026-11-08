# Traits are proposed as plain files

The skill teaches the agent to offer a trait when a correction sounds reusable, and to write nothing until the person agrees. With a yes it writes `TRAIT.md` and at least one `pass/` and one `fail/` case taken from the code just corrected, runs `traitor test <slug>`, and shows the numbers. A case that comes out wrong means the `expect` wording gets rewritten, not the case. If an existing trait already covers the rule, the agent adds a case to it instead of writing a near-copy.

Approval is the person's yes in chat and then reviewing and committing the change like any other file. crosshatch's approval form, MCP tools, and language model that splits and reconciles traits were rejected as machinery a diff review already covers.
