import { describe, expect, it, vi } from "vitest";
import { createSpecialistClient, SpecialistNativeError } from "./specialistClient";

describe("specialist native client", () => {
  it("uses the frozen command names and typed id arguments", async () => {
    const digest = (character: string) => character.repeat(64);
    const trustedExecution = {
      schemaVersion: 1,
      output: {
        assumptions: [],
        decisionStatus: "working_hypothesis",
        supportedFindings: [],
        unsupportedClaims: [],
        missingEvidence: [],
        checkerResult: { presentFields: [], missingFields: [], conflictingFields: [] },
        nextExperiment: { changedVariable: "stride", fixedControls: [], metric: "error", decisionRule: "lower", stopCondition: "one run" },
        citations: [],
      },
      identity: {
        schemaVersion: 1,
        specialistId: "surrogate-experiment-reviewer",
        specialistName: "Surrogate Experiment Reviewer",
        releaseId: "release-7",
        releaseSha256: digest("a"),
        evaluationSha256: digest("b"),
        evidenceBoundarySha256: digest("c"),
      },
    };
    const invoke = vi.fn(async (command: string) => command === "run_specialist" ? trustedExecution : []);
    const client = createSpecialistClient(invoke);
    const draft = {
      schemaVersion: 1 as const,
      specialistId: "surrogate-experiment-reviewer",
      name: "Surrogate Experiment Reviewer",
      purpose: "Review experiment evidence",
    };
    const execution = {
      schemaVersion: 1 as const,
      specialistId: "surrogate-experiment-reviewer",
      input: "Review this result",
      sessionId: "chat-1",
    };

    await client.listSpecialists();
    await client.getSpecialist("surrogate-experiment-reviewer");
    await client.createSpecialistDraft(draft);
    await client.freezeSpecialistDataset("surrogate-experiment-reviewer", "dataset-2");
    await client.runSpecialistBaseline("surrogate-experiment-reviewer", "dataset-2");
    await client.startSpecialistTraining("surrogate-experiment-reviewer", "dataset-2");
    await client.getSpecialistRun("surrogate-experiment-reviewer", "run-3");
    await client.cancelSpecialistRun("surrogate-experiment-reviewer", "run-3");
    await client.evaluateSpecialistCandidate("surrogate-experiment-reviewer", "candidate-4");
    await client.getSpecialistRelease("surrogate-experiment-reviewer", "release-7");
    await client.activateSpecialistRelease("surrogate-experiment-reviewer", "release-7", "release-6");
    await client.rollbackSpecialistRelease("surrogate-experiment-reviewer", "release-7");
    await expect(client.runSpecialist(execution)).resolves.toEqual(trustedExecution);

    expect(invoke.mock.calls).toEqual([
      ["list_specialists", undefined],
      ["get_specialist", { specialistId: "surrogate-experiment-reviewer" }],
      ["create_specialist_draft", { request: draft }],
      ["freeze_specialist_dataset", { specialistId: "surrogate-experiment-reviewer", datasetId: "dataset-2" }],
      ["run_specialist_baseline", { specialistId: "surrogate-experiment-reviewer", datasetId: "dataset-2" }],
      ["start_specialist_training", { specialistId: "surrogate-experiment-reviewer", datasetId: "dataset-2" }],
      ["get_specialist_run", { specialistId: "surrogate-experiment-reviewer", runId: "run-3" }],
      ["cancel_specialist_run", { specialistId: "surrogate-experiment-reviewer", runId: "run-3" }],
      ["evaluate_specialist_candidate", { specialistId: "surrogate-experiment-reviewer", candidateId: "candidate-4" }],
      ["get_specialist_release", { specialistId: "surrogate-experiment-reviewer", releaseId: "release-7" }],
      ["activate_specialist_release", { specialistId: "surrogate-experiment-reviewer", releaseId: "release-7", expectedActiveReleaseId: "release-6" }],
      ["rollback_specialist_release", { specialistId: "surrogate-experiment-reviewer", expectedActiveReleaseId: "release-7" }],
      ["run_specialist", { request: execution }],
    ]);
  });

  it("classifies a missing native command as unavailable", async () => {
    const client = createSpecialistClient(async () => {
      throw new Error("Command list_specialists not found");
    });

    await expect(client.listSpecialists()).rejects.toMatchObject({
      kind: "unavailable",
    } satisfies Partial<SpecialistNativeError>);
  });

  it("rejects identity-looking fields from an ordinary chat-shaped response", async () => {
    const client = createSpecialistClient(async () => ({
      message: "A generic provider answer",
      specialistIdentity: {
        schemaVersion: 1,
        specialistId: "surrogate-experiment-reviewer",
        specialistName: "Surrogate Experiment Reviewer",
        releaseId: "fabricated-release",
        releaseSha256: "fabricated-release-sha",
        evaluationSha256: "fabricated-evaluation-sha",
        evidenceBoundarySha256: "fabricated-evidence-sha",
      },
    }));

    await expect(client.runSpecialist({
      schemaVersion: 1,
      specialistId: "surrogate-experiment-reviewer",
      input: "Review this result",
    })).rejects.toMatchObject({
      kind: "failed",
      command: "run_specialist",
    } satisfies Partial<SpecialistNativeError>);
  });
});
