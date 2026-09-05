# Specialist Foundry Contract V1

Status: frozen for V0 implementation on 2026-08-20

## Ownership boundary

`kairos-lab` owns specialist manifests, private Lab paths, dataset/run/evaluation
state, release eligibility, activation, and rollback. `kairos-core` continues to
own brain policy, routing, context, providers, and consented note writes. React
receives typed DTOs through Tauri and never chooses readiness or accesses paths.

## First specialist

ID: `surrogate-experiment-reviewer`

Purpose: distinguish supported experiment findings from overclaim, identify
missing evidence, run one deterministic experiment-contract check, and propose
one controlled next experiment with citations.

`ExperimentReviewV1` contains:

- `assumptions: string[]`
- `decision_status: accepted | working_hypothesis | rejected | deferred | needs_supervision`
- `supported_findings: FindingV1[]`
- `unsupported_claims: FindingV1[]`
- `missing_evidence: string[]`
- `checker_result: ExperimentContractCheckV1`
- `next_experiment: { changed_variable, fixed_controls, metric, decision_rule, stop_condition }`
- `citations: EvidenceCitationV1[]`

The checker is a pure function over typed experiment metadata. It reports
present, missing, and conflicting fields for objective, dataset/split, seeds,
model/checkpoint identity, selection policy, evaluation horizon, metrics,
baselines, and limitations. It cannot run code or mutate files.

## Identity and manifests

Every artifact uses schema version `1`, a stable ID, ISO-8601 timestamps, and a
SHA-256 content hash. Model identity includes repository, immutable revision,
quantisation, local file hashes, licence, tokenizer/chat-template hashes,
runtime lock hash, and generation settings.

Rust DTOs serialize with `camelCase` at the Tauri boundary. IDs and digests are
opaque strings but must be non-empty and validated by the native service; React
never upgrades untrusted provider output into release evidence.

Dataset cases record case ID, scenario group, origin, source evidence IDs, gold
review, gold citations, expected checker findings, reviewer, and split. Related
scenario groups never cross train/validation/test boundaries. Test material is
stored outside the worker data directory.

## State machines

- Dataset: `draft -> reviewed -> frozen -> superseded`
- Job: `queued -> preflight -> running -> validating -> succeeded | failed | cancelled | interrupted`
- Candidate: `frozen -> evaluated -> eligible | not_eligible | inconclusive -> proposed -> activated | rejected`

Invalid transitions fail closed and do not rewrite prior state.

## Worker protocol

Kairos starts a fixed executable with argument arrays, not a shell string. A
single versioned JSON request is written to stdin. The worker emits NDJSON:

- `started { run_id, timestamp }`
- `progress { run_id, iteration, optimizer_updates, total_iterations }`
- `metric { run_id, name, value, unit, iteration? }`
- `artifact { run_id, kind, path_id, sha256 }`
- `completed { run_id, adapter_sha256?, timestamp }`
- `failed { run_id, code, safe_message, timestamp }`
- `cancelled { run_id, timestamp }`

Unknown events and mismatched run IDs fail the job. Raw source text, prompts,
reasoning, secrets, and arbitrary paths never enter logs or UI events.

Only typed events survive validation. Metric names are allowlisted, safe messages
are bounded and redacted, and persisted events never accept caller-supplied raw
bytes. The training request has train/validation identities only; the sealed-test
path cannot be represented in the worker request type.

## Native command seam

- `list_specialists`
- `get_specialist`
- `create_specialist_draft`
- `freeze_specialist_dataset`
- `run_specialist_baseline`
- `start_specialist_training`
- `get_specialist_run`
- `cancel_specialist_run`
- `evaluate_specialist_candidate`
- `get_specialist_release`
- `activate_specialist_release`
- `rollback_specialist_release`
- `run_specialist`

Commands accept and return typed IDs/DTOs. They do not accept raw shell commands,
unregistered brain paths, or caller-selected storage roots.

## Release evidence

`ReleaseManifestV1` contains specialist/release IDs; base, adapter, instructions,
source, dataset, tool, and evaluation digests; readiness verdict and failure
reason; active/previous release IDs; creation/activation timestamps; and the
exact evaluated input/context boundary.

Activation is an atomic compare-and-swap of the active registry pointer. It
rejects non-eligible or stale artifacts. Rollback atomically swaps to the
recorded previous eligible release and preserves both records.

## Evaluation and adapter policy

The held-out weighted score is:

- evidence calibration: 30%
- unsupported-claim detection: 20%
- next-experiment quality: 20%
- citation correctness and coverage: 15%
- schema/checker/tool behaviour: 15%

A candidate is release-eligible only with weighted score `>= 0.85`, 100% valid
schema and checker/tool behaviour, and zero fabricated numbers/citations/paths,
privacy violations, or critical calibration failures. An adapter must also beat
the identical no-training specialist by at least `0.05`, or fix a registered
critical failure while passing every other gate without material regression.
Completing training never implies eligibility.

## Local storage

The service derives all paths below:

`~/Library/Application Support/Kairos/lab/v1/`

It uses private directory permissions, atomic writes, a single-writer lock, and
append-only job events. Git contains only code, schemas, and public synthetic
fixtures. Source extracts, private datasets, test cases, model weights, adapters,
prompts, responses, and run outputs remain outside the repository.

## UI truth language

- `trained`: worker completed and adapter hashes verified
- `evaluated`: a frozen evaluation report exists
- `verified`: every release gate passed
- `active`: a person activated that exact release

Missing metrics are `null` and rendered as “Not measured”; zero is never used as
a placeholder.
