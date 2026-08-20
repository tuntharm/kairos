# kairos-lab

`kairos-lab` owns the Specialist Foundry V0 lifecycle boundary: versioned
manifests, fail-closed state machines, the pure experiment-contract checker,
held-out release eligibility, private derived storage, atomic active/previous
release pointers, rollback, and the bounded worker protocol.

This crate is intentionally a standalone Cargo workspace until the project lead
integrates it into the root workspace. It performs no model download or training
and never accepts caller-selected production storage roots or shell strings.

Run its tests without repository integration:

```sh
cargo test --offline --manifest-path crates/kairos-lab/Cargo.toml
```
