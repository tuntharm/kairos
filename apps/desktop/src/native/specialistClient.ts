import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  CandidateId,
  DatasetId,
  ReleaseId,
  ReleaseManifestV1,
  RunId,
  SpecialistComparisonV1,
  SpecialistDetailV1,
  SpecialistDraftRequestV1,
  SpecialistExecutionRequestV1,
  SpecialistExecutionV1,
  SpecialistId,
  SpecialistRunSummaryV1,
  SpecialistSummaryV1,
} from "./specialistContracts";

export type NativeInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;
export type SpecialistNativeErrorKind = "unavailable" | "failed";

export class SpecialistNativeError extends Error {
  readonly kind: SpecialistNativeErrorKind;
  readonly command: string;

  constructor(kind: SpecialistNativeErrorKind, command: string, message: string) {
    super(message);
    this.name = "SpecialistNativeError";
    this.kind = kind;
    this.command = command;
  }
}

function nativeError(command: string, reason: unknown): SpecialistNativeError {
  const message = reason instanceof Error ? reason.message : String(reason);
  const unavailable = /(?:command|handler).*(?:not found|unknown)|not (?:installed|available)|unknown (?:command|handler)/i.test(message);
  return new SpecialistNativeError(unavailable ? "unavailable" : "failed", command, message);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function parseRunSpecialistEnvelope(value: unknown, expectedSpecialistId: SpecialistId): SpecialistExecutionV1 {
  const envelope = isRecord(value) ? value : undefined;
  const identity = isRecord(envelope?.identity) ? envelope.identity : undefined;
  const identityFields = [
    "specialistId",
    "specialistName",
    "releaseId",
    "releaseSha256",
    "evaluationSha256",
    "evidenceBoundarySha256",
  ] as const;
  if (
    envelope?.schemaVersion !== 1
    || identity?.schemaVersion !== 1
    || !identityFields.every((field) => typeof identity[field] === "string" && identity[field].length > 0)
    || identity.specialistId !== expectedSpecialistId
    || !("output" in envelope)
  ) {
    throw new SpecialistNativeError("failed", "run_specialist", "The native run_specialist response did not match SpecialistExecutionV1.");
  }
  return {
    schemaVersion: 1,
    output: envelope.output,
    identity: {
      schemaVersion: 1,
      specialistId: identity.specialistId as string,
      specialistName: identity.specialistName as string,
      releaseId: identity.releaseId as string,
      releaseSha256: identity.releaseSha256 as string,
      evaluationSha256: identity.evaluationSha256 as string,
      evidenceBoundarySha256: identity.evidenceBoundarySha256 as string,
    },
  };
}

async function call<T>(invoke: NativeInvoke, command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke(command, args) as T;
  } catch (reason) {
    throw nativeError(command, reason);
  }
}

export type SpecialistClient = ReturnType<typeof createSpecialistClient>;

export function createSpecialistClient(invoke: NativeInvoke = (command, args) => tauriInvoke(command, args)) {
  return {
    listSpecialists: () => call<SpecialistSummaryV1[]>(invoke, "list_specialists"),
    getSpecialist: (specialistId: SpecialistId) => call<SpecialistDetailV1>(invoke, "get_specialist", { specialistId }),
    createSpecialistDraft: (request: SpecialistDraftRequestV1) => call<SpecialistDetailV1>(invoke, "create_specialist_draft", { request }),
    freezeSpecialistDataset: (specialistId: SpecialistId, datasetId: DatasetId) => call<SpecialistDetailV1>(invoke, "freeze_specialist_dataset", { specialistId, datasetId }),
    runSpecialistBaseline: (specialistId: SpecialistId, datasetId: DatasetId) => call<SpecialistRunSummaryV1>(invoke, "run_specialist_baseline", { specialistId, datasetId }),
    startSpecialistTraining: (specialistId: SpecialistId, datasetId: DatasetId) => call<SpecialistRunSummaryV1>(invoke, "start_specialist_training", { specialistId, datasetId }),
    getSpecialistRun: (specialistId: SpecialistId, runId: RunId) => call<SpecialistRunSummaryV1>(invoke, "get_specialist_run", { specialistId, runId }),
    cancelSpecialistRun: (specialistId: SpecialistId, runId: RunId) => call<SpecialistRunSummaryV1>(invoke, "cancel_specialist_run", { specialistId, runId }),
    evaluateSpecialistCandidate: (specialistId: SpecialistId, candidateId: CandidateId) => call<SpecialistComparisonV1>(invoke, "evaluate_specialist_candidate", { specialistId, candidateId }),
    getSpecialistRelease: (specialistId: SpecialistId, releaseId: ReleaseId) => call<ReleaseManifestV1>(invoke, "get_specialist_release", { specialistId, releaseId }),
    activateSpecialistRelease: (specialistId: SpecialistId, releaseId: ReleaseId) => call<ReleaseManifestV1>(invoke, "activate_specialist_release", { specialistId, releaseId }),
    rollbackSpecialistRelease: (specialistId: SpecialistId) => call<ReleaseManifestV1>(invoke, "rollback_specialist_release", { specialistId }),
    runSpecialist: async (request: SpecialistExecutionRequestV1) => parseRunSpecialistEnvelope(await call<unknown>(invoke, "run_specialist", { request }), request.specialistId),
  };
}
