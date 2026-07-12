# Local MCP setup

Build the binary first:

```bash
cargo build -p kairos-mcp
```

Then register the resulting `kairos-mcp` binary as a local stdio server in the
host of your choice. Pass `--config` only when you want a non-default registry.

The server is intentionally read-only and **route-only**. It never sends note
contents, file excerpts, or locally generated answers to the host, because
Kairos cannot verify whether a connected host ultimately uses a cloud model or
obtain the desktop app's per-turn consent.

It provides:

- `kairos_route`: recommend a connected brain for a question.
- `kairos_context`: return routing metadata plus the local-only boundary.
- `kairos_brief`: return daily-brief routing metadata only.
- `kairos_handoff`: return a copyable, route-only packet for another agent.

Use the Kairos desktop app or `kairos brief` for local Ollama synthesis. The
desktop app's consented cloud/API/CLI flow does not extend to MCP hosts.
