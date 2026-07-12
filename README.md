# Kairos

Kairos is a local-only developer alpha for Tharm's distributed personal knowledge
systems. It routes a question to authoritative notes, retrieves a few relevant
allowlisted notes, and uses a selected loopback Ollama model to produce a
grounded brief.

Kairos is not a second wiki and it does not share live chat memory between AI
tools. The durable files in the connected brains remain the source of truth.

## First working loop

1. Initialise the local Tharm profile.
2. Ask `What should I do next, and why?`.
3. Kairos reads only its trusted, allowlisted startup notes and shows the local
   sources it used.
4. Its built-in loopback Ollama adapter produces one cited next action with the
   explicitly selected local model.

## Local model settings

Kairos stores local inference settings in its existing registry at
`~/Library/Application Support/Kairos/brains.json`:

- Endpoint: `http://localhost:11434` only.
- Default: `qwen3.6:35b-mlx`.
- Fast router / manual fallback: `qwen3:8b`.
- Optional alternatives: `gpt-oss:20b` and `glm-4.7-flash`.
- Context window: 32,768 tokens (`num_ctx: 32768`).

The desktop selector persists the chosen model. Kairos checks that Ollama is
running and that the exact chosen model is installed before synthesis. It never
silently switches to another model; instead it shows the relevant `ollama serve`
or `ollama pull <model>` setup step. An untagged model may resolve only to its
explicit `:latest` tag (for example, `glm-4.7-flash` →
`glm-4.7-flash:latest`).

Context stays bounded: Kairos reads its registered router/current-context notes
first, then ranks at most three notes across the routed brains' explicit
`retrievalAllow` scopes. It never inserts a whole vault into a model prompt.

## Desktop surface

The desktop shell uses the vendored [Kairos design system](design/README.md):
its dark control plane, routing-state artwork, source citation chips, and
brand tokens keep the local-first boundary legible rather than decorative.

- The bundled application uses the supplied Kairos `.icns`, `.ico`, and
  alpha-safe PNG icon assets.
- On macOS, a template menu-bar icon appears while Kairos is running. Click it
  (or press `⌥ Space`) to show or hide the app; its menu has **Show Kairos**
  and **Quit Kairos** actions.
- The browser preview intentionally shows a disabled local-runtime state rather
  than attempting to call a native Tauri command. Vault access and synthesis
  happen only in the desktop app.

## Development

```bash
pnpm install
cargo test --workspace
cargo run -p kairos-cli -- init
cargo run -p kairos-cli -- doctor
cargo run -p kairos-cli -- model qwen3:8b
cargo run -p kairos-cli -- brief
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
- The selected model is persisted and verified before every local synthesis;
  there is no automatic model fallback.
- Ollama receives a 32K token window and only a bounded retrieval pack, never a
  whole vault.
- `90_Private`, `#private`, and `agent_access: explicit_only` are withheld.
- `startupAllow` is fail-closed: a startup file must be explicitly allowlisted
  before Kairos resolves or reads it.
- `local_only` note contents cannot leave through an MCP host. Per-turn cloud
  consent and redaction are future work, not a hidden fallback.
- The app never writes into a connected brain in this release.
- Chat transcripts are not persisted.
