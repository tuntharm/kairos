import { describe, expect, it } from "vitest";
import {
  atlasNodeDiameter,
  createAtlasPhysics,
  meaningfulConnectionDegrees,
  settleAtlasPhysics,
  type AtlasGraphEdge,
  type AtlasGraphNode,
} from "./atlasPhysics";

const nodes: AtlasGraphNode[] = [
  { id: "a", brainId: "one", kind: "note", x: 24, y: 26 },
  { id: "b", brainId: "one", kind: "note", x: 38, y: 34 },
  { id: "c", brainId: "two", kind: "bridge", x: 72, y: 68 },
  { id: "brain:one", brainId: "one", kind: "brain", x: 30, y: 30 },
];

const edges: AtlasGraphEdge[] = [
  { source: "a", target: "b", kind: "wiki_link" },
  { source: "a", target: "b", kind: "embed" },
  { source: "a", target: "c", kind: "cross_brain_bridge" },
  { source: "brain:one", target: "a", kind: "owns" },
  { source: "a", target: "c", kind: "tag" },
];

describe("atlas physics", () => {
  it("sizes nodes from unique meaningful neighbours only", () => {
    const degrees = meaningfulConnectionDegrees(nodes, edges);
    expect(degrees.get("a")).toBe(2);
    expect(degrees.get("b")).toBe(1);
    expect(degrees.get("c")).toBe(1);
    expect(degrees.get("brain:one")).toBe(0);
    expect(atlasNodeDiameter(nodes[0], 0)).toBe(10);
    expect(atlasNodeDiameter(nodes[0], 4)).toBe(15);
    expect(atlasNodeDiameter(nodes[3], 0)).toBe(13);
    expect(atlasNodeDiameter(nodes[0], 999)).toBe(26);
  });

  it("keeps a dragged node pinned while neighbouring nodes settle", () => {
    const controller = createAtlasPhysics(nodes, edges, {});
    expect(controller).not.toBeNull();
    const before = settleAtlasPhysics(controller!, 112);
    controller!.pin("a", { x: 82, y: 78 });
    const after = settleAtlasPhysics(controller!, 160);

    expect(after.a).toEqual({ x: 82, y: 78 });
    expect(after.b).not.toEqual(before.b);
    Object.values(after).forEach((position) => {
      expect(position.x).toBeGreaterThanOrEqual(2);
      expect(position.x).toBeLessThanOrEqual(98);
      expect(position.y).toBeGreaterThanOrEqual(2);
      expect(position.y).toBeLessThanOrEqual(98);
    });
  });

  it("normalizes an oversized saved world before simulation", () => {
    const controller = createAtlasPhysics([
      { id: "far-a", brainId: "one", kind: "note", x: -555, y: -615 },
      { id: "far-b", brainId: "two", kind: "note", x: 733, y: 721 },
    ], [{ source: "far-a", target: "far-b", kind: "wiki_link" }], {});
    expect(controller).not.toBeNull();
    const positions = settleAtlasPhysics(controller!, 1);
    const values = Object.values(positions);
    expect(Math.min(...values.map((position) => position.x))).toBeGreaterThan(-12);
    expect(Math.max(...values.map((position) => position.x))).toBeLessThan(112);
    expect(Math.min(...values.map((position) => position.y))).toBeGreaterThan(-12);
    expect(Math.max(...values.map((position) => position.y))).toBeLessThan(112);
  });
});
