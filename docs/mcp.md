# Local MCP setup

Build the binary first:

```bash
cargo build -p kairos-mcp
```

Then register the resulting `kairos-mcp` binary as a local stdio server in the
host of your choice. Pass `--config` only when you want a non-default registry.

The server is intentionally read-only and **route-only** in this alpha. It
never sends note contents, file excerpts, or locally generated answers to the
host, because Kairos cannot verify whether a connected host ultimately uses a
cloud model.

It provides:

- `kairos_route`: recommend a connected brain for a question.
- `kairos_context`: return routing metadata plus the local-only boundary.
- `kairos_brief`: return daily-brief routing metadata only.
- `kairos_handoff`: return a copyable, route-only packet for another agent.

Use the Kairos desktop app or `kairos brief` for local Ollama synthesis. A
future MCP egress workflow will need a visible, per-turn user confirmation and
redaction support before it can transfer governed context to another AI.
