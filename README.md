# Kairos

Kairos is a macOS-first, chat-first control surface for a distributed personal
knowledge system. It routes a question to approved Obsidian brains, retrieves a
small cited context pack, and lets Tharm choose a local model, a consented cloud
provider, or an explicitly invoked coding CLI.

Kairos is not a second wiki. Connected notes remain the source of truth; its
local chat history is a convenience, not shared memory for every AI tool.

## Core experience

- Press `⌥ Space` to open a compact, persistent chat. Expand it into **Chat**,
  **What Next**, **Brain Map**, and **Settings**.
- **What Next** starts with a cross-brain pulse. A question that cannot be
  routed safely asks for a brain instead of broadcasting it.
- Replies identify the routed brain(s), cite retrieved note paths, and show the
  state of any temporary attachments.
- Closing can hide the app; the summon shortcut, launch-at-login, and whether
  to reopen compact chat or the last surface are settings.

## Local model setup

Kairos connects to Ollama only at `http://localhost:11434`. It detects whether
Ollama is missing, installed but stopped, or running; shows available disk,
model package size, 32K-context compatibility, download progress, cancel/retry,
and a model test. If Ollama is absent it opens the official macOS installer;
Kairos never asks a normal user to run a Terminal command and does not bundle
Ollama or any model in its DMG.

- Starter default: `qwen3.6:35b-mlx`
- Fast manual choice: `qwen3:8b`
- Optional choices: `gpt-oss:20b`, `glm-4.7-flash`
- Context: 32,768 tokens, kept by retrieval rather than lowering context or
  inserting a whole vault
- Memory budget: `Auto` detects the Mac (48 GB on this development Mac), with
  `16 / 24 / 32 / 48 / 64 / 96 / 192 GB / Custom` choices. It is a planning
  budget, not a statement about installed RAM.

The selected model and context are always explicit. Kairos never silently
switches models, falls back to another provider, or reduces the context window.

## Brains, privacy, and atlas

Add a brain through the native folder picker, then confirm its name, routing
hints, retrieval scope, graph inclusion, egress policy, and write policy.
Kairos indexes in place and does not copy or own vault content.

- `90_Private`, `#private`, and `agent_access: explicit_only` are withheld from
  retrieval, graph construction, and external context unless a future
  path-scoped grant explicitly allows them.
- The progressive Brain Map keeps Kairos at the centre and displays separate
  brain clusters, explicit Markdown/wiki links, tags, and bridges. It caches
  graph metadata only; it does not infer semantic links in v1.
- Offline, moved, withheld, and capped sources remain visible as diagnostics.

## Providers and consent

The provider layer is modular: local **Ollama**, direct **OpenAI API**, direct
**Anthropic API**, installed **Codex CLI**, and installed **Claude Code CLI**.
API keys live only in macOS Keychain. Coding-CLI handoffs are text-only,
ephemeral, read-only, and require the already-installed CLI to be authenticated.

Before every non-local turn, Kairos previews the exact outbound message,
temporary-file extraction, selected note excerpts, destination, and model. A
preview can be cancelled. `local_only` brains cannot send material through a
cloud/API/CLI provider, and there is no provider or model fallback.

## Chat, uploads, and note proposals

Chats persist locally. A chat may attach up to five `.md`, `.txt`, `.pdf`, or
`.docx` files of up to 20 MB each. Extraction stays in memory, is visibly marked
temporary, and is discarded after the turn or when removed.

Kairos can only propose Markdown **create** or **edit** operations in a brain's
explicitly allowed directories. It shows the target path, a bounded diff, and
before/after hashes; confirmation uses a short-lived one-time nonce and detects
external changes. It cannot delete, move, create directories, or expose a raw
filesystem/shell interface to the WebView.

## Configuration

`~/Library/Application Support/Kairos/brains.json` uses schema v3. The app
atomically migrates a v2 registry and keeps `brains.v2.backup.json` beside it.
The schema records app behaviour, typed provider configuration without secrets,
brain policy, and local-model setup state.

## Development

```bash
pnpm install
cargo fmt --check
cargo test --workspace
pnpm --filter @kairos/desktop build
pnpm --filter @kairos/desktop tauri dev
```

Build a debug macOS bundle with:

```bash
pnpm --filter @kairos/desktop tauri build --debug
```

## MCP

`kairos-mcp` remains a local, route-only stdio bridge. It returns routing
metadata rather than note contents or model answers, because an MCP host cannot
give Kairos the per-turn consent guarantee provided by the desktop app. See
[docs/mcp.md](docs/mcp.md).
