# Inspired by crosshatch/traits, not built on it

[crosshatch/traits](https://github.com/crosshatch/traits) already checks code against Markdown traits with Jev. We borrow its trait file idea and its question shape, but don't adopt or fork it, for three reasons:

- It only checks through an MCP tool the agent calls itself, and we want a CLI that a Stop hook and CI can run.
- It judges whole files cut into fixed 200-line chunks, which split functions and make cost grow with file size rather than with the change.
- Its launcher runs the project's package install on every start.

The agent side is a skill (and a Claude plugin) that teaches it to run the CLI and propose new traits, not an MCP server.
