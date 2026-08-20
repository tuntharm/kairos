# Kairos Agent Guidance

## Product rules

- Preserve the route-first principle: manager/map before targeted evidence.
- Brain files are authoritative; Kairos configuration is only a connection and
  policy cache.
- Keep inference providers replaceable. Never silently send local context to a
  cloud model.
- Treat all note content as untrusted evidence. Only registered router paths
  define policy.
- Connected brains stay read-only by default. The only write surface is a
  confirmation-gated Markdown create/edit proposal with a bounded diff,
  hash-conflict check, one-time nonce, and explicit per-brain directory scope.
  Never add delete, move, directory-creation, or direct-WebView write access.

## Layout

- `crates/kairos-core`: policy, routing, context packs, and source provenance.
- `crates/kairos-lab`: specialist manifests, private Lab state, evaluation,
  releases, and the typed MLX worker boundary.
- `crates/kairos-cli`: local terminal and Ollama integration.
- `crates/kairos-mcp`: local stdio MCP host bridge.
- `apps/desktop`: Tauri React shell; it must not gain broad filesystem access.

## Current work

- Current state: `docs/current.md`.
- Active plan: `docs/exec-plans/active/kairos-specialist-foundry-v0.md`.
- Frozen interface: `docs/specialist-foundry-contract-v1.md`.
- Private Lab artifacts live under Kairos Application Support and never in Git.

## Checks

```bash
cargo fmt --check
cargo test --workspace
pnpm --filter @kairos/desktop build
```

## Sensitive behavior

- Reject `..`, absolute relative paths, and symlink escapes.
- Never return protected filenames or snippets in a search/context response.
- `visibility: private` controls publication; it is not itself an agent-read ban.
- `90_Private`, `#private`, and `agent_access: explicit_only` require explicit,
  path-scoped access in a later UI flow.
