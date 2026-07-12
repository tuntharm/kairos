# Architecture

```text
Tauri WebView (typed commands only) ────────┐
native desktop boundary ────────────────────┼──> kairos-core ──> registered brains
CLI + route-only MCP ───────────────────────┘         │
                                                       ├── Ollama loopback
                                                       ├── consented OpenAI / Anthropic API
                                                       └── explicit read-only Codex / Claude CLI
```

`kairos-core` owns routing, path validation, private-note exclusion, bounded
context packing, source provenance, provider requests, graph metadata parsing,
and confirmation-gated note writes. The WebView cannot resolve arbitrary paths,
run shell commands, or write a file directly.

## Per-turn flow

1. Route to at most two enabled brains. Ambiguous routes request a choice.
2. Apply the registered read policy before returning a filename, excerpt, or
   body; private and explicit-only sources fail closed.
3. Build a bounded context pack from manager/current-context files and ranked
   retrieval-allowlisted notes, never the whole vault.
4. For Ollama, call the selected loopback model with `num_ctx: 32768`.
5. For OpenAI, Anthropic, Codex CLI, or Claude CLI, create a one-time preview
   containing the exact outbound material and require explicit confirmation.
   `local_only` content is denied before preview or dispatch.
6. Persist source citations and local chat metadata; discard temporary upload
   extraction after use.

## Native command boundary

Tauri exposes narrow typed commands for folder selection and inspection, model
health/pull/test, chat streaming, provider settings, graph snapshots, upload
extraction, consented write proposals, and shortcut rebinding. It does not pass
arbitrary paths, shell strings, or filesystem capabilities from the WebView.

## Graph and writes

Graph indexing is deliberately separate from retrieval. It uses only explicit
Markdown/wiki links, tags, and configured bridges, then saves metadata in the
Kairos application-support directory. Semantic-link inference is out of scope.

Writes are limited to confirmation-gated Markdown creates and edits within a
brain's configured directories. A proposal contains a bounded diff and hashes;
the native store keeps the full body in memory until one-time confirmation.
Conflict, expiry, private-policy, path traversal, and symlink checks are
re-evaluated immediately before the atomic write.
