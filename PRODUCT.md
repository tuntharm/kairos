# Kairos Product

## One line

Kairos is Tharm's summonable, local-first front door to distributed brains:
ask once, get authoritative context, one useful next action, and the sources
behind it.

## Alpha user and job

The first user is Tharm on this Mac. The alpha must answer:

> What should I do next, and why?

without preloading whole vaults, inventing current state, or writing back to a
brain.

## Local inference policy

- Ollama runs only at `http://localhost:11434`.
- `qwen3.6:35b-mlx` is the saved default local model.
- `qwen3:8b` is a fast, explicitly chosen router/manual-fallback option—not an
  automatic downgrade.
- `gpt-oss:20b` and `glm-4.7-flash` are optional alternatives.
- The selected model and 32K token context setting live in local Kairos
  settings. Kairos verifies both Ollama and the selected tag before use.
- Retrieval starts from routed, allowlisted notes and caps the inserted evidence;
  it never preloads a whole vault.

## Product boundary

Kairos owns routing, context packing, provenance, and safety policy. It does
not own the notes, replace Obsidian, or become a live shared consciousness for
Codex, Claude, and Cursor.

## Non-goals for the alpha

- Accounts, sync, telemetry, or hosted backends.
- Generic multi-user onboarding.
- Cloud inference or automatic cloud fallback.
- Hidden model substitution when a selected local model is unavailable.
- Note mutations, attachment ingestion, wake words, or autonomous coding.
- Parsing every connected vault into an index.

## Success metrics

- At least 80% correct brain choice without a manual correction.
- At least 80% of real answers rated useful and grounded.
- A useful next action in under 60 seconds.
- Used on at least five of seven pilot days.
- Zero protected-note or egress-policy violations.
