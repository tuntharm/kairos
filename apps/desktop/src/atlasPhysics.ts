import {
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  type Force,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";

export type AtlasGraphNode = {
  id: string;
  brainId: string;
  clusterId?: string;
  kind?: string;
  x?: number;
  y?: number;
};

export type AtlasGraphEdge = {
  id?: string;
  source: string;
  target: string;
  kind?: string;
};

export type AtlasPosition = { x: number; y: number };
export type AtlasPositions = Record<string, AtlasPosition>;

type AtlasForceNode = AtlasGraphNode & SimulationNodeDatum & {
  x: number;
  y: number;
  diameter: number;
  clusterX: number;
  clusterY: number;
};

type AtlasForceEdge = Omit<AtlasGraphEdge, "source" | "target"> & SimulationLinkDatum<AtlasForceNode>;

const meaningfulEdgeKinds = new Set([
  "wiki_link",
  "markdown_link",
  "embed",
  "cross_brain_bridge",
  "bridge",
]);

const maxAtlasCoordinate = 98;
const minAtlasCoordinate = 2;

export function clampAtlasCoordinate(value: number) {
  return Math.max(minAtlasCoordinate, Math.min(maxAtlasCoordinate, value));
}

function edgeKind(edge: Pick<AtlasGraphEdge, "kind">) {
  return edge.kind?.toLowerCase() ?? "";
}

export function meaningfulConnectionDegrees(nodes: AtlasGraphNode[], edges: AtlasGraphEdge[]) {
  const knownNodes = new Set(nodes.map((node) => node.id));
  const seenPairs = new Set<string>();
  const degrees = new Map(nodes.map((node) => [node.id, 0]));

  edges.forEach((edge) => {
    if (!meaningfulEdgeKinds.has(edgeKind(edge)) || edge.source === edge.target) return;
    if (!knownNodes.has(edge.source) || !knownNodes.has(edge.target)) return;
    const pair = [edge.source, edge.target].sort().join("\u0000");
    if (seenPairs.has(pair)) return;
    seenPairs.add(pair);
    degrees.set(edge.source, (degrees.get(edge.source) ?? 0) + 1);
    degrees.set(edge.target, (degrees.get(edge.target) ?? 0) + 1);
  });

  return degrees;
}

export function atlasNodeDiameter(node: Pick<AtlasGraphNode, "kind">, degree: number) {
  const base = node.kind === "brain" ? 13 : 10;
  return Math.min(26, Number((base + Math.sqrt(Math.max(0, degree)) * 2.5).toFixed(1)));
}

function linkDistance(edge: AtlasForceEdge) {
  switch (edgeKind(edge)) {
    case "owns": return 26;
    case "contains": return 10;
    case "tag": return 11;
    case "cross_brain_bridge":
    case "bridge": return 29;
    default: return 15;
  }
}

function linkStrength(edge: AtlasForceEdge) {
  switch (edgeKind(edge)) {
    case "owns": return 0.03;
    case "contains": return 0.08;
    case "tag": return 0.035;
    case "cross_brain_bridge":
    case "bridge": return 0.025;
    default: return 0.065;
  }
}

function isKairosNode(node: AtlasGraphNode) {
  return node.id === "kairos" || node.kind === "kairos";
}

function stableHash(value: string) {
  let hash = 2_166_136_261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16_777_619);
  }
  return hash >>> 0;
}

function seededUnit(value: string) {
  return stableHash(value) / 4_294_967_295;
}

function seedPosition(node: AtlasGraphNode, pinnedPositions: AtlasPositions) {
  const pinned = pinnedPositions[node.id];
  return {
    x: clampAtlasCoordinate(pinned?.x ?? node.x ?? 50),
    y: clampAtlasCoordinate(pinned?.y ?? node.y ?? 50),
  };
}

function clusterCentres(nodes: AtlasGraphNode[]) {
  const brainIds = Array.from(new Set(nodes.map((node) => node.brainId || node.clusterId || "unassigned")))
    .sort((left, right) => left.localeCompare(right));
  const phase = seededUnit(brainIds.join("|")) * Math.PI * 2;
  const radius = brainIds.length <= 1 ? 0 : Math.min(22, 14 + brainIds.length * 2);
  return new Map(brainIds.map((brainId, index) => {
    const angle = phase + ((Math.PI * 2 * index) / brainIds.length);
    return [brainId, {
      x: 50 + Math.cos(angle) * radius,
      y: 50 + Math.sin(angle) * radius,
    }];
  }));
}

function atlasBoundsForce(): Force<AtlasForceNode, undefined> {
  let nodes: AtlasForceNode[] = [];
  const min = 4;
  const max = 96;
  const force = (() => {
    nodes.forEach((node) => {
      if (node.fx === undefined || node.fx === null) {
        if (node.x < min) {
          node.x = min;
          node.vx = Math.max(0, node.vx ?? 0) * 0.35;
        } else if (node.x > max) {
          node.x = max;
          node.vx = Math.min(0, node.vx ?? 0) * 0.35;
        }
      }
      if (node.fy === undefined || node.fy === null) {
        if (node.y < min) {
          node.y = min;
          node.vy = Math.max(0, node.vy ?? 0) * 0.35;
        } else if (node.y > max) {
          node.y = max;
          node.vy = Math.min(0, node.vy ?? 0) * 0.35;
        }
      }
    });
  }) as Force<AtlasForceNode, undefined>;
  force.initialize = (nextNodes) => {
    nodes = nextNodes;
  };
  return force;
}

export type AtlasPhysicsController = {
  simulation: Simulation<AtlasForceNode, undefined>;
  positions: () => AtlasPositions;
  pin: (nodeId: string, position: AtlasPosition) => void;
  movePinnedNode: (nodeId: string, position: AtlasPosition) => void;
  reheat: () => void;
  settle: () => void;
  stop: () => void;
};

export function createAtlasPhysics(
  graphNodes: AtlasGraphNode[],
  graphEdges: AtlasGraphEdge[],
  pinnedPositions: AtlasPositions,
): AtlasPhysicsController | null {
  const nodes = graphNodes.filter((node) => !isKairosNode(node));
  if (nodes.length === 0) return null;

  const degrees = meaningfulConnectionDegrees(nodes, graphEdges);
  const centres = clusterCentres(nodes);
  const forceNodes: AtlasForceNode[] = nodes.map((node) => {
    const position = seedPosition(node, pinnedPositions);
    const centre = centres.get(node.brainId || node.clusterId || "unassigned") ?? position;
    const pinned = pinnedPositions[node.id];
    return {
      ...node,
      ...position,
      fx: pinned?.x,
      fy: pinned?.y,
      diameter: atlasNodeDiameter(node, degrees.get(node.id) ?? 0),
      clusterX: centre.x,
      clusterY: centre.y,
    };
  });
  const nodeIds = new Set(forceNodes.map((node) => node.id));
  const forceEdges: AtlasForceEdge[] = graphEdges
    .filter((edge) => nodeIds.has(edge.source) && nodeIds.has(edge.target) && edge.source !== edge.target)
    .map((edge) => ({ ...edge }));
  const nodeById = new Map(forceNodes.map((node) => [node.id, node]));

  const simulation = forceSimulation(forceNodes)
    .force("link", forceLink<AtlasForceNode, AtlasForceEdge>(forceEdges)
      .id((node) => node.id)
      .distance(linkDistance)
      .strength(linkStrength))
    .force("charge", forceManyBody<AtlasForceNode>().strength(-6).distanceMax(26))
    .force("collide", forceCollide<AtlasForceNode>()
      .radius((node) => 1 + node.diameter / 12)
      .strength(0.9)
      .iterations(2))
    .force("cluster-x", forceX<AtlasForceNode>((node) => node.clusterX)
      .strength((node) => node.kind === "brain" ? 0.09 : 0.065))
    .force("cluster-y", forceY<AtlasForceNode>((node) => node.clusterY)
      .strength((node) => node.kind === "brain" ? 0.09 : 0.065))
    .force("bounds", atlasBoundsForce())
    .alphaDecay(0.052)
    .velocityDecay(0.48);

  const positions = (): AtlasPositions => Object.fromEntries(forceNodes.map((node) => [node.id, {
    x: clampAtlasCoordinate(node.x),
    y: clampAtlasCoordinate(node.y),
  }]));
  const movePinnedNode = (nodeId: string, position: AtlasPosition) => {
    const node = nodeById.get(nodeId);
    if (!node) return;
    const x = clampAtlasCoordinate(position.x);
    const y = clampAtlasCoordinate(position.y);
    node.fx = x;
    node.fy = y;
    node.x = x;
    node.y = y;
  };
  const pin = (nodeId: string, position: AtlasPosition) => {
    movePinnedNode(nodeId, position);
  };

  return {
    simulation,
    positions,
    pin,
    movePinnedNode,
    reheat: () => simulation.alphaTarget(0.12).alpha(Math.max(simulation.alpha(), 0.72)).restart(),
    settle: () => simulation.alphaTarget(0).alpha(Math.max(simulation.alpha(), 0.42)).restart(),
    stop: () => simulation.stop(),
  };
}

export function settleAtlasPhysics(controller: AtlasPhysicsController, ticks = 112) {
  controller.simulation.stop();
  controller.simulation.tick(ticks);
  return controller.positions();
}
