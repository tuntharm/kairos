# Architecture

```text
Codex / Claude Code / Cursor --stdio MCP (route metadata only)---┐
Kairos CLI + selected loopback Ollama adapter --------------------┼--> kairos-core --> registered brains
Tauri React shell ------------------------------------------------┘
```

`kairos-core` is the only layer allowed to resolve brain paths, apply privacy
policy, read source text, and create provenance records. The desktop WebView
only receives typed context-pack data.

The alpha is deterministic before it is generative:

1. Classify and select at most two enabled brains.
2. Read only registered router/current-context paths.
3. Apply access policy before returning a filename, snippet, or body.
4. Build a size-bounded `ContextPack`: registered router/current-context notes,
   then at most three ranked notes from an explicit retrieval allowlist.
5. Send content only to the selected loopback Ollama adapter at
   `http://localhost:11434`, with `num_ctx: 32768`; MCP receives route metadata
   only until a per-turn egress-consent workflow exists.

`OllamaProvider` is a provider-specific adapter. Future Codex, Claude, or other
cloud adapters can be added beside it, behind an explicit egress-consent flow,
without changing the routing, retrieval, or policy core.

Cloud providers will be added as an explicit, previewed egress action. They do
not belong in the local alpha's code path.
