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
- [ ] Gate 1: freeze shared contracts, storage boundary, and worker protocol.
- [ ] Gate 2: pass the synthetic pipeline smoke without downloading a model.
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

## Resume point

Define failing tests for the specialist lifecycle and worker protocol, implement
the narrow Lab crate seam, then let the two disjoint workstreams proceed.

