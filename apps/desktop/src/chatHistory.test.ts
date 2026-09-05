import { describe, expect, it } from "vitest";
import { hydrateStoredChatMessage } from "./chatHistory";

describe("local chat history hydration", () => {
  it("retains exact native specialist release identity after reload", () => {
    const identity = {
      schemaVersion: 1 as const,
      specialistId: "surrogate-experiment-reviewer",
      specialistName: "Surrogate Experiment Reviewer",
      releaseId: "release-001",
      releaseSha256: "a".repeat(64),
      evaluationSha256: "b".repeat(64),
      evidenceBoundarySha256: "c".repeat(64),
    };
    const hydrated = hydrateStoredChatMessage({
      id: "message-1",
      role: "assistant",
      content: "Decision: working hypothesis",
      createdAt: "2026-08-20T00:00:00Z",
      sourceIds: ["phd:evidence-1"],
      routeBrainIds: ["phd"],
      specialistIdentity: identity,
    }, (id) => id === "phd" ? "PhD Research" : id);

    expect(hydrated.specialistIdentity).toEqual(identity);
    expect(hydrated.routes).toEqual([{ id: "phd", name: "PhD Research" }]);
    expect(hydrated.citations[0]).toMatchObject({ brainId: "phd", relativePath: "evidence-1" });
  });
});
