import { describe, expect, it } from "vitest";
import { resolveChatDelivery } from "./chatTarget";

const activeSpecialist = {
  schemaVersion: 1 as const,
  specialistId: "surrogate-experiment-reviewer",
  name: "Surrogate Experiment Reviewer",
  purpose: "Review experiment evidence",
  activeReleaseId: "release-7",
  previousReleaseId: null,
};

describe("chat delivery resolution", () => {
  it("keeps an active specialist local even when the Manager provider is cloud", () => {
    const result = resolveChatDelivery(
      "specialist:surrogate-experiment-reviewer",
      [activeSpecialist],
      true,
    );

    expect(result.kind).toBe("specialist_local");
    expect(result.specialist).toEqual(activeSpecialist);
    expect(result.badge).toBe("LOCAL SPECIALIST");
  });

  it("fails closed when the selected specialist is no longer active", () => {
    const result = resolveChatDelivery(
      "specialist:surrogate-experiment-reviewer",
      [],
      true,
    );

    expect(result.kind).toBe("specialist_unavailable");
    expect(result.specialist).toBeUndefined();
    expect(result.badge).toBe("SPECIALIST PAUSED");
  });

  it("uses the Manager provider only after Manager is explicitly selected", () => {
    expect(resolveChatDelivery("manager", [], true).kind).toBe("manager_external");
    expect(resolveChatDelivery("manager", [], false).kind).toBe("manager_local");
  });
});
