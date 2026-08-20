import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { SpecialistNativeError } from "../../native/specialistClient";
import {
  SpecialistFoundryView,
  formatMeasuredMetric,
  loadSpecialistFoundryState,
} from "./SpecialistFoundry";

describe("Specialist Foundry presentation", () => {
  it("exposes the five truthful Lab tabs", () => {
    const html = renderToStaticMarkup(
      <SpecialistFoundryView
        activeTab="specialists"
        onTabChange={() => undefined}
        state={{ status: "empty" }}
        runtimeSetup={<div>Ollama setup stays here</div>}
      />,
    );

    for (const label of ["Specialists", "Data &amp; Tests", "Runs", "Compare", "Releases"]) {
      expect(html).toContain(label);
    }
    expect(html).toContain("No specialists yet");
    expect(html).toContain("Ollama setup stays here");
  });

  it("offers an explicit no-model draft action only in the empty native state", () => {
    const html = renderToStaticMarkup(
      <SpecialistFoundryView
        activeTab="specialists"
        onTabChange={() => undefined}
        state={{ status: "empty" }}
        onCreateDraft={() => undefined}
      />,
    );

    expect(html).toContain("Create Surrogate Reviewer draft");
    expect(html).toContain("does not read a brain, run a model, download anything, or train");
  });

  it("distinguishes loading, unavailable, and failed states", () => {
    const render = (state: Parameters<typeof SpecialistFoundryView>[0]["state"]) => renderToStaticMarkup(
      <SpecialistFoundryView activeTab="runs" onTabChange={() => undefined} state={state} />,
    );

    expect(render({ status: "loading" })).toContain("Loading specialist state");
    expect(render({ status: "unavailable", message: "Native commands are not installed." })).toContain("Native commands are not installed.");
    expect(render({ status: "failed", message: "Lab state could not be read." })).toContain("Lab state could not be read.");
  });

  it("renders null metrics as Not measured while preserving a measured zero", () => {
    expect(formatMeasuredMetric({ name: "Tokens per second", value: null, unit: "tok/s" })).toBe("Not measured");
    expect(formatMeasuredMetric({ name: "Failed checks", value: 0, unit: null })).toBe("0");
  });

  it("exposes explicit activation and rollback controls only from recorded release state", () => {
    const digest = (character: string) => character.repeat(64);
    const specialist = {
      schemaVersion: 1 as const,
      specialistId: "surrogate-experiment-reviewer",
      name: "Surrogate Experiment Reviewer",
      purpose: "Review experiment evidence",
      activeReleaseId: "release-active",
      previousReleaseId: "release-previous",
      datasets: [],
      tests: [],
      runs: [],
      comparisons: [],
      releases: [{
        schemaVersion: 1 as const,
        specialistId: "surrogate-experiment-reviewer",
        releaseId: "release-next",
        baseSha256: digest("a"),
        adapterSha256: digest("b"),
        instructionsSha256: digest("c"),
        sourceSha256: digest("d"),
        datasetSha256: digest("e"),
        toolSha256: digest("f"),
        evaluationId: "evaluation-4",
        evaluationSha256: digest("1"),
        evaluatedBoundarySha256: digest("2"),
        readinessVerdict: "eligible" as const,
        failureReason: null,
        activeReleaseId: "release-active",
        previousReleaseId: "release-previous",
        createdAt: "2026-08-20T10:00:00Z",
        activatedAt: null,
      }],
    };
    const html = renderToStaticMarkup(<SpecialistFoundryView
      activeTab="releases"
      onTabChange={() => undefined}
      state={{ status: "ready", specialists: [specialist], selected: specialist }}
      onActivateRelease={() => undefined}
      onRollbackRelease={() => undefined}
    />);

    expect(html).toContain("Activate release-next");
    expect(html).toContain("Roll back to release-previous");
    expect(html).toContain(digest("2"));
  });

  it("maps native lifecycle results to honest empty, ready, unavailable, and failed states", async () => {
    const emptyClient = {
      listSpecialists: vi.fn(async () => []),
      getSpecialist: vi.fn(),
    };
    await expect(loadSpecialistFoundryState(emptyClient)).resolves.toEqual({ status: "empty" });
    expect(emptyClient.getSpecialist).not.toHaveBeenCalled();

    const specialist = {
      schemaVersion: 1 as const,
      specialistId: "surrogate-experiment-reviewer",
      name: "Surrogate Experiment Reviewer",
      purpose: "Review experiment evidence",
      activeReleaseId: null,
      previousReleaseId: null,
      datasets: [],
      tests: [],
      runs: [],
      comparisons: [],
      releases: [],
    };
    await expect(loadSpecialistFoundryState({
      listSpecialists: vi.fn(async () => [specialist]),
      getSpecialist: vi.fn(async () => specialist),
    })).resolves.toEqual({ status: "ready", specialists: [specialist], selected: specialist });

    await expect(loadSpecialistFoundryState({
      listSpecialists: vi.fn(async () => {
        throw new SpecialistNativeError("unavailable", "list_specialists", "Native commands are not installed.");
      }),
      getSpecialist: vi.fn(),
    })).resolves.toMatchObject({ status: "unavailable" });

    await expect(loadSpecialistFoundryState({
      listSpecialists: vi.fn(async () => { throw new Error("read failed"); }),
      getSpecialist: vi.fn(),
    })).resolves.toMatchObject({ status: "failed" });
  });
});
