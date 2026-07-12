# Architecture

```text
Codex / Claude Code / Cursor --stdio MCP (route metadata only)---┐
Kairos CLI + loopback Ollama -------------------------------------┼--> kairos-core --> registered brains
Tauri React shell ------------------------------------------------┘
```

`kairos-core` is the only layer allowed to resolve brain paths, apply privacy
policy, read source text, and create provenance records. The desktop WebView
only receives typed context-pack data.

The alpha is deterministic before it is generative:

1. Classify and select at most two enabled brains.
2. Read only registered router/current-context paths.
3. Apply access policy before returning a filename, snippet, or body.
4. Build a size-bounded `ContextPack` with runtime-generated source IDs.
5. Send content only to the loopback Ollama adapter; MCP receives route metadata
   only until a per-turn egress-consent workflow exists.

Cloud providers will be added as an explicit, previewed egress action. They do
not belong in the local alpha's code path.
