# Kairos Current State

Verified: 2026-08-20

## Objective

Build one real, inspectable local specialist inside Kairos before another broad
desktop redesign. The first specialist is **Surrogate Experiment Reviewer**.

## Current stage

`specialist-foundry-v0 / integrated-safety-scaffold / manual-gate`

The earlier chat, Atlas, routing, provider, Ollama, history, and consented-write
foundation is preserved in local commit `2231c51`. The pre-foundry binary patch
is `/private/tmp/kairos-pre-foundry-2026-08-20.patch` with SHA-256
`85d9f6e97a52e131b2a5d6a8d3c9340d06fcc417bdc9e194109bb5eb994ac32d`.

## Verified working scaffold

- `kairos-lab` is a workspace crate with versioned specialist, dataset, run,
  evaluation, release, activation, rollback, storage, and managed-worker
  contracts.
- The Lab store is derived below private Kairos Application Support storage;
  React cannot choose a filesystem root or invoke an arbitrary process.
- Twelve repository-safe synthetic cases validate the draft/smoke path with
  zero model calls and zero training iterations.
- All 13 typed Lab commands are registered in Tauri. Operations that require
  sources, a model, sealed tests, or training return explicit typed gated errors;
  they do not invent state or fall back to a Manager provider.
- The desktop Lab has Specialists, Data, Runs, Compare, and Releases views. Chat
  distinguishes Manager from a human-activated specialist and pins the exact
  displayed release ID in its future execution request.
- `cargo test --workspace --offline`: 95 tests passed.
- `pnpm --dir apps/desktop test`: 20 tests passed.
- `python3 -m unittest ...`: 4 worker-contract tests passed.
- Rust formatting, workspace Clippy with warnings denied, TypeScript checks,
  Vite production build, and `git diff --check` passed.
- No model weights, private source extracts, datasets, adapters, or run outputs
  are tracked in Git.

## Decisions

- Kairos remains the provider-neutral brain manager and release authority.
- Local specialist lifecycle belongs in a separate `kairos-lab` crate.
- The first local base candidate is a pinned 4-bit Qwen3 4B MLX artifact.
- Improvement order is instructions, approved retrieval, typed checker, then an
  educational QLoRA experiment.
- Exact approved PhD excerpts may leave the local machine only after a per-packet
  outgoing-context preview and explicit approval.
- A completed training job is not a verified or active release.

## Current boundary

This is a **safety scaffold**, not a trained or active specialist. Dataset
freezing, real baseline inference, MLX-LM training, sealed evaluation, and
specialist execution remain deliberately fail-closed until their manual gates.
The worker currently implements only a deterministic `SmokeNoop` protocol.

Hardware preflight on 2026-08-20 confirmed an Apple M4 Pro with 48 GB unified
memory and 462 GiB free disk. System Python is 3.14.6; the managed MLX-LM
runtime and model are absent.

See `docs/exec-plans/active/kairos-specialist-foundry-v0.md`.

## Next gate

1. Tharm approves the exact PhD excerpts that may form the local-only evidence
   pack and 120-case dataset; no broad vault ingestion.
2. Separately approve creation of a managed Python 3.12/MLX-LM runtime and the
   pinned `mlx-community/Qwen3-4B-4bit` download at immutable revision
   `4dcb3d101c2a062e5c1d4bb173588c54ea6c4d25`.
3. Run a 20-iteration local smoke and report measured time and memory before
   requesting approval for the 300-iteration educational QLoRA run.
