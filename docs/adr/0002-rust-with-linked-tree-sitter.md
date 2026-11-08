# Rust, with tree-sitter linked into the binary

The user's other tools are Effect TypeScript compiled with Bun, and that was the first plan, with `oxc-parser` for the syntax tree. A compiled Bun binary with oxc-parser embedded came out at 111MB, almost all of it Bun's runtime, and oxc doesn't parse Markdown, so docs would need a second, different splitter.

We write it in Rust and link tree-sitter grammars into the binary instead. One small binary parses code and Markdown the same way, and nothing has to be installed beside it. The tree-sitter CLI as a runtime requirement was rejected: every user would also have to install and compile each grammar. Go was rejected because its tree-sitter binding needs cgo, which makes cross-compiling harder.

What we give up is Effect and `@effect/ai-typesafe`. TypeSafe has no Rust SDK, so traitor calls its HTTP API directly; that's one POST endpoint.
