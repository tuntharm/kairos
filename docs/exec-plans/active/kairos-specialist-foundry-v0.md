# Kairos Specialist Foundry V0

Owner: Project Lead / Integrator

Status: active

Started: 2026-08-20

## Outcome

Run a real local Surrogate Experiment Reviewer, expose its exact release and
evidence in compact Chat, and prove that activation and rollback persist across
relaunch. Complete one bounded educational QLoRA lesson without promoting an
adapter that loses to the no-training specialist.

## Acceptance criteria

- Versioned specialist, source, dataset, run, evaluation, and release contracts.
- Private artifacts stored only under Kairos Application Support.
- Frozen train/validation/test grouping with the sealed test excluded from the
  training worker.
- Untouched-base, no-training, and optional-adapter comparison under an identical
  pinned runtime.
- Atomic activation and rollback with authoritative active/previous release IDs.
- Compact Chat runs the active release and shows release/evidence identity.
- No automatic source egress, model download, training, promotion, or fallback.

## Constraints

- Preserve the pre-foundry app checkpoint `2231c51`.
- One canonical integration owner; Team 1 and Team 2 write only disjoint paths.
- No private notes, source extracts, datasets, weights, adapters, or run outputs
  enter Git.
- No arbitrary shell or filesystem access is exposed to React.
- The website prototype, public release, deployment, and MCP expansion are out
  of scope for this execution plan.

## Gates

- [x] Gate 0: preserve current app and record the passing baseline.
- [x] Gate 1: freeze shared contracts, storage boundary, and worker protocol.
- [x] Gate 2: pass the synthetic pipeline smoke without downloading a model.
- [ ] Gate 3: freeze approved sources, dataset groups, and no-training baseline.
- [ ] Gate 4: show download/memory/time preflight and request approval for QLoRA.
- [ ] Gate 5: run sealed evaluation and propose the strongest eligible release.
- [ ] Gate 6: integrate real Lab state, compact Chat execution, activation, and rollback.
- [ ] Gate 7: run full verification, archive this plan, and update project memory.

## Manual approval gates

Stop before exact-source egress, model download, the full QLoRA run, opening the
sealed test, activation, or rollback. Report destination, artifact identity,
disk/memory/time estimate, and consequence before asking.

## Progress and evidence

### 2026-08-20

- Pre-foundry state preserved in commit `2231c51`.
- Patch SHA-256:
  `85d9f6e97a52e131b2a5d6a8d3c9340d06fcc417bdc9e194109bb5eb994ac32d`.
- Baseline: 57 Rust tests and 3 frontend tests passed; production frontend built.
- Contracts and scaffold integrated through commits `ec58b88`, `439b55f`, and
  `53b8528`; the Lab crate is now a root workspace member.
- The deterministic public-fixture smoke records 12 cases, zero model calls,
  and zero training iterations. It does not claim baseline quality.
- Tauri registers all 13 typed Lab commands. Dataset, inference, training,
  sealed evaluation, and execution commands fail closed until their required
  evidence and approvals exist.
- Activation/rollback CAS, release eligibility gates, 72/18/30 group isolation,
  physical sealed-test separation, worker integrity/correlation/cancellation,
  and durable job lifecycle are covered by unit tests.
- Compact Chat now has an explicit Manager/specialist target. An unavailable or
  inactive specialist stays paused rather than silently switching to Manager,
  and future execution pins the displayed release ID.
- Current verification: 95 Rust tests, 20 frontend tests, and 4 Python worker
  tests passed; Clippy with warnings denied and the production frontend build
  passed.
- Local preflight: M4 Pro, 48 GB unified memory, 462 GiB free disk, Python
  3.14.6. Managed Python 3.12/MLX-LM and model weights are absent.
- Official artifact check: `mlx-community/Qwen3-4B-4bit`, immutable revision
  `4dcb3d101c2a062e5c1d4bb173588c54ea6c4d25`, Apache-2.0, 2.28 GB repository
  total with a 2.26 GB safetensors file.

## Resume point

Pause at the first real-data/runtime gate. Obtain Tharm's exact approved source
excerpts for local-only dataset preparation, then separately request approval
for the managed Python 3.12/MLX-LM runtime and pinned 2.28 GB model download.
After installation, run only the 20-iteration smoke and report measured runtime
and memory before proposing the full 300-iteration lesson.
