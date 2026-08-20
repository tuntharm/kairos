# Kairos Current State

Verified: 2026-08-20

## Objective

Build one real, inspectable local specialist inside Kairos before another broad
desktop redesign. The first specialist is **Surrogate Experiment Reviewer**.

## Current stage

`specialist-foundry-v0 / contract-and-foundation`

The earlier chat, Atlas, routing, provider, Ollama, history, and consented-write
foundation is preserved in local commit `2231c51`. The pre-foundry binary patch
is `/private/tmp/kairos-pre-foundry-2026-08-20.patch` with SHA-256
`85d9f6e97a52e131b2a5d6a8d3c9340d06fcc417bdc9e194109bb5eb994ac32d`.

## Verified working baseline

- `cargo test --workspace`: 57 tests passed.
- `pnpm --dir apps/desktop test`: 3 tests passed.
- `pnpm --dir apps/desktop run build`: TypeScript and Vite production build passed.
- The repository contains no specialist, dataset, MLX worker, evaluation,
  activation, or rollback implementation yet.

## Decisions

- Kairos remains the provider-neutral brain manager and release authority.
- Local specialist lifecycle belongs in a separate `kairos-lab` crate.
- The first local base candidate is a pinned 4-bit Qwen3 4B MLX artifact.
- Improvement order is instructions, approved retrieval, typed checker, then an
  educational QLoRA experiment.
- Exact approved PhD excerpts may leave the local machine only after a per-packet
  outgoing-context preview and explicit approval.
- A completed training job is not a verified or active release.

## Active work

Project Lead owns shared contracts and integration. Team 1 owns the Lab engine
and worker. Team 2 owns desktop stabilization and the Lab presentation. The
independent verifier remains read-only.

See `docs/exec-plans/active/kairos-specialist-foundry-v0.md`.

## Next gate

Freeze and test the shared domain, worker, evaluation, activation, and rollback
contracts before parallel implementation begins.

