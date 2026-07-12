# Kairos

Kairos is a local-only developer alpha for Tharm's distributed personal knowledge
systems. It routes a question to authoritative notes, builds a bounded local
context pack, and uses a loopback Ollama model to produce a grounded brief.

Kairos is not a second wiki and it does not share live chat memory between AI
tools. The durable files in the connected brains remain the source of truth.

## First working loop

1. Initialise the local Tharm profile.
2. Ask `What should I do next, and why?`.
3. Kairos reads only its trusted, allowlisted startup notes and shows the local
   sources it used.
4. Its built-in loopback Ollama adapter produces one cited next action.

## Development

```bash
pnpm install
cargo test --workspace
cargo run -p kairos-cli -- init
cargo run -p kairos-cli -- doctor
cargo run -p kairos-cli -- brief --offline
pnpm --filter @kairos/desktop tauri dev
```

The initial profile is intentionally personal and points at Tharm's existing
router and Everyday/PhD brains. It is a Tharm-only developer profile, not
generic onboarding. A later release can add a picker/importer for portable
brains.

## MCP

Run a local stdio server after building:

```bash
cargo run -p kairos-mcp
```

It exposes route-only `kairos_route`, `kairos_context`, `kairos_brief`, and
`kairos_handoff` tools. In this alpha, MCP never returns note text or model
answers; see [docs/mcp.md](docs/mcp.md) for the exact boundary.

## Privacy defaults

- The local model is the default. Cloud use is not implemented in this alpha.
- `90_Private`, `#private`, and `agent_access: explicit_only` are withheld.
- `startupAllow` is fail-closed: a startup file must be explicitly allowlisted
  before Kairos resolves or reads it.
- `local_only` note contents cannot leave through an MCP host. Per-turn cloud
  consent and redaction are future work, not a hidden fallback.
- The app never writes into a connected brain in this release.
- Chat transcripts are not persisted.
