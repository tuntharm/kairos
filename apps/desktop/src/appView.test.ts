import { describe, expect, it } from "vitest";
import { expandedWorkspace } from "./appView";

describe("desktop view transitions", () => {
  it("opens Atlas when compact Chat expands", () => {
    expect(expandedWorkspace("chat")).toBe("atlas");
  });

  it("preserves an already selected workspace", () => {
    expect(expandedWorkspace("lab")).toBe("lab");
    expect(expandedWorkspace("settings")).toBe("settings");
  });
});
