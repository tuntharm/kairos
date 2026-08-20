import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { SpecialistResponseIdentity } from "./SpecialistResponseIdentity";

describe("specialist response identity", () => {
  it("shows the exact specialist, release, and evidence boundary", () => {
    const html = renderToStaticMarkup(<SpecialistResponseIdentity identity={{
      schemaVersion: 1,
      specialistId: "surrogate-experiment-reviewer",
      specialistName: "Surrogate Experiment Reviewer",
      releaseId: "release-7",
      releaseSha256: "release-sha",
      evaluationSha256: "evaluation-sha",
      evidenceBoundarySha256: "evidence-sha",
    }} />);

    expect(html).toContain("Surrogate Experiment Reviewer");
    expect(html).toContain("release-7");
    expect(html).toContain("evidence-sha");
  });
});
