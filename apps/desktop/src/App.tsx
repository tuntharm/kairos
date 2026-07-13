import {
  type CSSProperties,
  type PointerEvent as ReactPointerEvent,
  type WheelEvent as ReactWheelEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import brandMark from "../../../design/assets/brand/kairos-mark-gradient.svg";
import brandWordmark from "../../../design/assets/brand/kairos-wordmark-dark.svg";
import fileCitation from "../../../design/assets/citations/file.svg";
import routeError from "../../../design/assets/routing/error.svg";
import routeIdle from "../../../design/assets/routing/idle.svg";
import routeMulti from "../../../design/assets/routing/routed-multi.svg";
import routeOne from "../../../design/assets/routing/routed-one.svg";

type HardwareProfile = "auto" | "apple_unified" | "nvidia_vram" | "cpu_only";

type OllamaModelChoice = { id: string; label: string; role: string };

type AppStatus = {
  configPath: string;
  initialized: boolean;
  model: {
    endpoint: string;
    selectedModel: string;
    resolvedModel?: string;
    contextWindowTokens: number;
    running: boolean;
    selectedModelInstalled: boolean;
    installedModels: string[];
    setupMessage?: string;
    choices: OllamaModelChoice[];
    selectedModelFit?: ModelFit;
  };
  app?: {
    summonShortcut?: string;
    summonTarget?: "compact_chat" | "last_surface";
    launchAtLogin?: boolean;
    closeToHide?: boolean;
    keepAboveOtherWindows?: boolean;
  };
  providers?: Array<{
    id: string;
    label: string;
    enabled: boolean;
    configured: boolean;
    detail: string;
  }>;
};

type Source = {
  source: {
    id: string;
    brainId: string;
    relativePath: string;
    modifiedAt?: string;
  };
  content: string;
  truncated: boolean;
};

type ContextPack = {
  query: string;
  route: {
    brains: Array<{ id: string; name: string; reason: string }>;
    requiresChoice: boolean;
  };
  sources: Source[];
  withheldSources: Array<{ brainId: string; reason: string }>;
  freshnessWarnings: string[];
};

type BriefAnswer = {
  action: string;
  why: string;
  caveat: string;
  sourceIds: string[];
};

type LocalBrief = {
  answer: BriefAnswer;
  context: ContextPack;
};

type ModelFit = {
  fit: "recommended" | "tight" | "not_recommended" | "unknown";
  budgetGb?: number | null;
  minimumMemoryGb?: number | null;
  recommendedMemoryGb?: number | null;
  contextWindowTokens?: number;
  maximumContextTokens?: number | null;
  contextCompatible?: boolean | null;
  requiresTest?: boolean;
  message: string;
};

type LocalSetupModel = {
  id: string;
  label?: string;
  role?: string;
  installed?: boolean;
  downloadSize?: string;
  memoryBand?: string;
  recommendedContext?: string;
  fit?: ModelFit;
  capabilities?: string[];
  variant?: string;
  whyRecommended?: string;
  recommendationRank?: number;
  defaultRecommended?: boolean;
  advancedOnly?: boolean;
  verifiedAt32k?: boolean;
};

type OllamaInstallAction = {
  command?: string;
  label?: string;
  officialUrl?: string;
  needed?: boolean;
};

type LocalSetupStatus = {
  ollamaInstalled?: boolean;
  running?: boolean;
  endpoint?: string;
  detectedMemoryGb?: number;
  detectedHardwareProfile?: HardwareProfile;
  selectedHardwareProfile?: HardwareProfile;
  effectiveHardwareProfile?: HardwareProfile;
  hardwareProfileLabel?: string;
  primaryFitLimitGb?: number | null;
  primaryFitLimitLabel?: string;
  hardwarePlanningOverride?: boolean;
  availableDiskGb?: number;
  selectedModelInstalled?: boolean;
  setupMessage?: string;
  contextWindowTokens?: number;
  memoryBudgetMode?: "auto" | "preset" | "custom";
  memoryBudgetGb?: number | null;
  effectiveMemoryBudgetGb?: number | null;
  memoryBudgetMessage?: string;
  memoryBudgetPlanningOnly?: boolean;
  selectedModelFit?: ModelFit;
  ollamaInstallAction?: OllamaInstallAction;
  models?: LocalSetupModel[];
  recommendedModels?: LocalSetupModel[];
  advancedModels?: LocalSetupModel[];
};

type OllamaPullProgress = {
  model: string;
  status: string;
  completed?: number;
  total?: number;
  percent?: number;
};

type BrainRecord = {
  id: string;
  name: string;
  role: string;
  rootPath?: string;
  enabled?: boolean;
  graphEnabled?: boolean;
  egressPolicy?: string;
  writePolicy?: string;
  status?: "ready" | "offline" | "indexing" | "restricted";
  noteCount?: number;
};

type BrainPolicyDraft = {
  brainId: string;
  egressPolicy: "local_only" | "cloud_allowed" | "redact_required";
  writePolicy: "readonly" | "confirm_every_write" | "prohibited";
  graphEnabled: boolean;
};

type GraphNode = {
  id: string;
  label: string;
  brainId: string;
  clusterId?: string;
  relativePath?: string;
  kind?: string;
  x?: number;
  y?: number;
  protected?: boolean;
};

type GraphEdge = { id?: string; source: string; target: string; kind?: string };

type GraphSnapshot = {
  nodes: GraphNode[];
  edges: GraphEdge[];
  indexedAt?: string;
  stale?: boolean;
};

type Attachment = {
  id: string;
  name: string;
  size: number;
  type: string;
};

type Citation = {
  id: string;
  brainId: string;
  relativePath: string;
  modifiedAt?: string;
};

type ChatMessage = {
  id: string;
  role: "user" | "assistant";
  body: string;
  createdAt: string;
  routes?: Array<{ id: string; name: string }>;
  citations?: Citation[];
  provider?: string;
  notice?: boolean;
};

type StreamingAssistant = {
  turnId: string;
  body: string;
  createdAt: string;
};

type ChatPreview = {
  previewToken?: string;
  message: string;
  providerId: string;
  providerLabel: string;
  routes: Array<{ id: string; name: string }>;
  citations: Citation[];
  attachmentNames: string[];
  outgoingSummary: string;
};

type NoteWriteProposal = {
  id: string;
  nonce: string;
  brainId: string;
  relativePath: string;
  kind: "create" | "edit";
  beforeSha256?: string;
  afterSha256: string;
  diff: string;
  expiresAt: string;
};

type ChatSessionSummary = {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
  messageCount: number;
};

type StoredChatSession = ChatSessionSummary & {
  messages: Array<{
    id: string;
    role: "user" | "assistant";
    content: string;
    createdAt: string;
    sourceIds?: string[];
    providerLabel?: string;
    routeBrainIds?: string[];
  }>;
};

type ProviderId = "ollama" | "openai" | "anthropic" | "codex-cli" | "claude-cli";
type ViewId = "chat" | "next" | "map" | "settings";
type NavId = ViewId | "history" | "brains" | "privacy";
type SurfaceMode = "compact" | "cockpit";
type SettingsAnchor = "top" | "brains" | "privacy" | "writes";
type MemoryBudget = "auto" | "custom" | 16 | 24 | 32 | 48 | 64 | 96 | 192;
type KairosConsentMode = "ask" | "approve_local" | "full_kairos";
type ScopedExecutionGrant = { token: string; brainId: string; providerId: ProviderId; sessionId: string; expiresAt: string };
type GraphCamera = { scale: number; x: number; y: number };

type BrainVisual = { label: string; color: string; icon: string };

const briefQuestion = "What should I do next, and why?";
const ollamaInstallUrl = "https://ollama.com/download/mac";
const graphCameraFit: GraphCamera = { scale: 1, x: 0, y: 0 };
const graphZoomMin = 0.6;
const graphZoomMax = 3;

const browserPreviewStatus: AppStatus = {
  configPath: "Browser preview",
  initialized: true,
  model: {
    endpoint: "http://localhost:11434",
    selectedModel: "qwen3.6:35b-mlx",
    resolvedModel: "qwen3.6:35b-mlx",
    contextWindowTokens: 32_768,
    running: true,
    selectedModelInstalled: true,
    installedModels: ["qwen3.6:35b-mlx", "qwen3:8b"],
    choices: [
      { id: "qwen3.6:35b-mlx", label: "Qwen 3.6 35B MLX", role: "Default local model" },
      { id: "qwen3:8b", label: "Qwen 3 8B", role: "Fast router" },
      { id: "gpt-oss:20b", label: "GPT-OSS 20B", role: "Optional alternative" },
      { id: "glm-4.7-flash", label: "GLM 4.7 Flash", role: "Optional alternative" },
    ],
  },
};

const browserPreviewSetup: LocalSetupStatus = {
  ollamaInstalled: true,
  running: true,
  endpoint: "http://localhost:11434",
  detectedMemoryGb: 48,
  detectedHardwareProfile: "apple_unified",
  selectedHardwareProfile: "auto",
  effectiveHardwareProfile: "apple_unified",
  hardwareProfileLabel: "Apple Silicon · auto-detected",
  primaryFitLimitGb: 48,
  primaryFitLimitLabel: "Apple unified memory",
  hardwarePlanningOverride: false,
  availableDiskGb: 532,
  selectedModelInstalled: true,
  contextWindowTokens: 32_768,
  memoryBudgetMode: "auto",
  memoryBudgetGb: null,
  effectiveMemoryBudgetGb: 48,
  memoryBudgetMessage: "Auto uses detected unified memory as a planning budget. It does not change the model or context.",
  memoryBudgetPlanningOnly: true,
};

const fallbackGraph: GraphSnapshot = {
  nodes: [{ id: "kairos", label: "Kairos", brainId: "kairos", kind: "kairos", x: 50, y: 49 }],
  edges: [],
};

const modelCatalog: Record<string, { label: string; role: string; downloadSize: string; memoryBand: string; context: string }> = {
  "qwen3.6:35b-mlx": {
    label: "Qwen 3.6 35B MLX",
    role: "Starter default",
    downloadSize: "~22 GB",
    memoryBand: "Power · 48 GB+ recommended",
    context: "32K context target",
  },
  "qwen3:8b": {
    label: "Qwen 3 8B",
    role: "Fast router",
    downloadSize: "~5.2 GB",
    memoryBand: "Light · 16 GB+",
    context: "32K context target",
  },
  "gpt-oss:20b": {
    label: "GPT-OSS 20B",
    role: "Optional alternative",
    downloadSize: "~14 GB",
    memoryBand: "Balanced · 32 GB+",
    context: "32K context target",
  },
  "glm-4.7-flash": {
    label: "GLM 4.7 Flash",
    role: "Optional alternative",
    downloadSize: "~19 GB",
    memoryBand: "Balanced · 32 GB+",
    context: "32K context target",
  },
};

const modelKnownWarnings: Record<string, string> = {
  "glm-4.7-flash": "Requires Ollama 0.14.3 pre-release or newer before download/test.",
};

function normalizedModelId(model: string) {
  return model.trim().replace(/:latest$/, "");
}

function sameModelId(left: string, right: string) {
  return normalizedModelId(left) === normalizedModelId(right);
}

function isInstalledModel(model: string, installedModels: string[]) {
  return installedModels.some((installed) => sameModelId(installed, model));
}

function modelFitLabel(fit?: ModelFit) {
  switch (fit?.fit) {
    case "recommended": return fit.requiresTest ? "Test required" : "Recommended";
    case "tight": return fit.requiresTest ? "Test required" : "Tight fit";
    case "not_recommended": return "Not recommended";
    default: return "No fit estimate";
  }
}

function modelFitTone(fit?: ModelFit): "success" | "warning" | "danger" | "neutral" {
  switch (fit?.fit) {
    case "recommended": return fit.requiresTest ? "warning" : "success";
    case "tight": return "warning";
    case "not_recommended": return "danger";
    default: return "neutral";
  }
}

function modelFitRank(fit?: ModelFit) {
  switch (fit?.fit) {
    case "recommended": return 0;
    case "tight": return 1;
    case "unknown": return 2;
    case "not_recommended": return 3;
    default: return 2;
  }
}

const providers: Array<{ id: ProviderId; label: string; detail: string; external: boolean }> = [
  { id: "ollama", label: "Ollama · local", detail: "Your Mac · no cloud egress", external: false },
  { id: "openai", label: "OpenAI API", detail: "Cloud · preview each turn", external: true },
  { id: "anthropic", label: "Anthropic API", detail: "Cloud · preview each turn", external: true },
  { id: "codex-cli", label: "Codex CLI", detail: "Explicit, ephemeral handoff", external: true },
  { id: "claude-cli", label: "Claude Code CLI", detail: "Explicit, ephemeral handoff", external: true },
];

const kairosConsentModes: Array<{ id: KairosConsentMode; label: string; detail: string }> = [
  {
    id: "ask",
    label: "Ask for approval",
    detail: "Current alpha default: sensitive egress and every note write require confirmation.",
  },
  {
    id: "approve_local",
    label: "Approve for me",
    detail: "Local chat is already read-only. Cloud/CLI turns and note writes still require their own confirmation.",
  },
  {
    id: "full_kairos",
    label: "Full access · scoped",
    detail: "A 30-minute, brain-scoped session grant. It never grants shell, arbitrary Mac access, private notes, deletion, or cloud/CLI egress without the normal preview.",
  },
];

const brainVisuals: Record<string, BrainVisual> = {
  kairos: { label: "Kairos", color: "#FFC766", icon: fileCitation },
};

const fileVisual: BrainVisual = { label: "File", color: "#A5B4CC", icon: fileCitation };
const dynamicBrainColors = ["#F28CCB", "#7CCB8C", "#FF9B73", "#8EA8FF", "#C6A4FF", "#E5C45B"];

function stableHash(value: string) {
  let hash = 2_166_136_261;
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16_777_619);
  }
  return hash >>> 0;
}

function labelForBrainId(brainId: string) {
  return brainId
    .replace(/^brain:/, "")
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (letter) => letter.toUpperCase());
}

function hasNativeBridge() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

function brainVisual(brainId: string): BrainVisual {
  const knownVisual = brainVisuals[brainId];
  if (knownVisual) return knownVisual;
  if (!brainId || brainId === "unassigned") return fileVisual;
  return {
    label: labelForBrainId(brainId),
    color: dynamicBrainColors[stableHash(brainId) % dynamicBrainColors.length],
    icon: fileCitation,
  };
}

function compactPath(path: string) {
  const pieces = path.split("/");
  return pieces.length > 2 ? pieces.slice(-2).join("/") : path;
}

function shortDate(iso?: string) {
  if (!iso) return undefined;
  const parsed = new Date(iso);
  return Number.isNaN(parsed.valueOf()) ? undefined : parsed.toLocaleDateString();
}

function makeId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function sourceCitation(source: Source["source"]): Citation {
  return {
    id: source.id,
    brainId: source.brainId,
    relativePath: source.relativePath,
    modifiedAt: source.modifiedAt,
  };
}

function defaultChatMessages(): ChatMessage[] {
  return [{
    id: "welcome",
    role: "assistant",
    body: "I’m Kairos. Ask what matters now, or use What Next for a cross-brain pulse. I route before I read.",
    createdAt: new Date().toISOString(),
    notice: true,
  }];
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? value as Record<string, unknown> : null;
}

function normalizeBrains(value: unknown): BrainRecord[] | null {
  const candidate = Array.isArray(value)
    ? value
    : Array.isArray(asRecord(value)?.brains)
      ? asRecord(value)?.brains as unknown[]
      : null;
  if (!candidate) return null;
  const brains = candidate.flatMap((item): BrainRecord[] => {
    const record = asRecord(item);
    if (!record || typeof record.id !== "string" || typeof record.name !== "string") return [];
    return [{
      id: record.id,
      name: record.name,
      role: typeof record.role === "string" ? record.role : "Registered brain",
      rootPath: typeof record.rootPath === "string" ? record.rootPath : undefined,
      enabled: typeof record.enabled === "boolean" ? record.enabled : true,
      graphEnabled: typeof record.graphEnabled === "boolean" ? record.graphEnabled : true,
      egressPolicy: typeof record.egressPolicy === "string" ? record.egressPolicy : "local_only",
      writePolicy: typeof record.writePolicy === "string" ? record.writePolicy : "readonly",
      status: ["ready", "offline", "indexing", "restricted"].includes(String(record.status))
        ? record.status as BrainRecord["status"]
        : "ready",
      noteCount: typeof record.noteCount === "number" ? record.noteCount : undefined,
    }];
  });
  return brains.length > 0 ? brains : null;
}

function canConfirmWrites(brain: BrainRecord) {
  return brain.writePolicy === "confirm_every_write" || brain.writePolicy === "propose_confirm";
}

function policyDraftFor(brain: BrainRecord): BrainPolicyDraft {
  return {
    brainId: brain.id,
    egressPolicy: brain.egressPolicy === "cloud_allowed" || brain.egressPolicy === "redact_required"
      ? brain.egressPolicy
      : "local_only",
    writePolicy: brain.writePolicy === "confirm_every_write" || brain.writePolicy === "prohibited"
      ? brain.writePolicy
      : "readonly",
    graphEnabled: brain.graphEnabled !== false,
  };
}

function graphBrainKey(node: GraphNode) {
  const candidate = node.brainId || node.clusterId || "kairos";
  return candidate.replace(/^brain:/, "") || "kairos";
}

function isKairosGraphNode(node: GraphNode) {
  return node.id === "kairos" || node.kind === "kairos";
}

function clampGraphCoordinate(value: number) {
  return Math.max(1, Math.min(99, value));
}

function seededUnit(value: string) {
  return stableHash(value) / 4_294_967_295;
}

function layoutGraph(nodes: GraphNode[], edges: GraphEdge[]) {
  const orderedNodes = [...nodes].sort((left, right) => left.id.localeCompare(right.id));
  if (orderedNodes.length < 2) return orderedNodes;

  const brainIds = Array.from(new Set(orderedNodes
    .filter((node) => !isKairosGraphNode(node))
    .map(graphBrainKey)))
    .sort((left, right) => left.localeCompare(right));
  const layoutSeed = orderedNodes.map((node) => node.id).join("|");
  const phase = seededUnit(`clusters:${layoutSeed}`) * Math.PI * 2;
  const clusterRadius = brainIds.length <= 1 ? 16 : Math.min(24, 15 + brainIds.length * 2);
  const clusters = new Map<string, { x: number; y: number }>();
  brainIds.forEach((brainId, index) => {
    const angle = phase + ((Math.PI * 2 * index) / brainIds.length);
    clusters.set(brainId, {
      x: 50 + Math.cos(angle) * clusterRadius,
      y: 50 + Math.sin(angle) * clusterRadius,
    });
  });

  const positions = orderedNodes.map((node) => {
    const fixed = isKairosGraphNode(node);
    const hasX = typeof node.x === "number" && Number.isFinite(node.x);
    const hasY = typeof node.y === "number" && Number.isFinite(node.y);
    const brainId = graphBrainKey(node);
    const cluster = clusters.get(brainId) ?? { x: 50, y: 50 };
    const angle = seededUnit(`${node.id}:angle`) * Math.PI * 2;
    const isBrainNode = node.kind === "brain";
    const spread = isBrainNode
      ? 2.5 + seededUnit(`${node.id}:spread`) * 2.5
      : 6 + seededUnit(`${node.id}:spread`) * 13;
    return {
      x: fixed ? 50 : hasX ? node.x! : clampGraphCoordinate(cluster.x + Math.cos(angle) * spread),
      y: fixed ? 50 : hasY ? node.y! : clampGraphCoordinate(cluster.y + Math.sin(angle) * spread),
      vx: 0,
      vy: 0,
      lockedX: fixed || hasX,
      lockedY: fixed || hasY,
      clusterX: cluster.x,
      clusterY: cluster.y,
    };
  });

  const nodeIndex = new Map(orderedNodes.map((node, index) => [node.id, index]));
  const edgePairs = edges
    .flatMap((edge) => {
      const source = nodeIndex.get(edge.source);
      const target = nodeIndex.get(edge.target);
      return source === undefined || target === undefined || source === target ? [] : [{ source, target, edge }];
    })
    .sort((left, right) => `${left.edge.source}:${left.edge.target}:${left.edge.kind ?? ""}`.localeCompare(`${right.edge.source}:${right.edge.target}:${right.edge.kind ?? ""}`));
  const count = orderedNodes.length;
  const iterations = count > 350 ? 64 : count > 160 ? 78 : 96;
  const forceX = new Float64Array(count);
  const forceY = new Float64Array(count);
  const repulsion = count > 350 ? 3.4 : 4.2;

  for (let step = 0; step < iterations; step += 1) {
    forceX.fill(0);
    forceY.fill(0);

    for (let source = 0; source < count; source += 1) {
      for (let target = source + 1; target < count; target += 1) {
        let dx = positions[source].x - positions[target].x;
        let dy = positions[source].y - positions[target].y;
        let distanceSquared = dx * dx + dy * dy;
        if (distanceSquared < 0.01) {
          const angle = seededUnit(`${orderedNodes[source].id}:${orderedNodes[target].id}`) * Math.PI * 2;
          dx = Math.cos(angle) * 0.1;
          dy = Math.sin(angle) * 0.1;
          distanceSquared = 0.01;
        }
        const distance = Math.sqrt(distanceSquared);
        const magnitude = repulsion / (distanceSquared + 0.8);
        const scale = magnitude / distance;
        const fx = dx * scale;
        const fy = dy * scale;
        if (!positions[source].lockedX) forceX[source] += fx;
        if (!positions[source].lockedY) forceY[source] += fy;
        if (!positions[target].lockedX) forceX[target] -= fx;
        if (!positions[target].lockedY) forceY[target] -= fy;
      }
    }

    for (const { source, target, edge } of edgePairs) {
      const sourcePosition = positions[source];
      const targetPosition = positions[target];
      const dx = targetPosition.x - sourcePosition.x;
      const dy = targetPosition.y - sourcePosition.y;
      const distance = Math.max(Math.sqrt(dx * dx + dy * dy), 0.01);
      const kind = edge.kind?.toLowerCase() ?? "";
      const sourceBrain = graphBrainKey(orderedNodes[source]);
      const targetBrain = graphBrainKey(orderedNodes[target]);
      const explicitLink = ["wiki_link", "markdown_link", "embed"].includes(kind);
      const crossBrain = explicitLink && sourceBrain !== targetBrain && sourceBrain !== "kairos" && targetBrain !== "kairos";
      const desiredLength = kind === "owns" ? 27 : crossBrain ? 29 : kind === "contains" ? 10 : 15;
      const strength = kind === "owns" ? 0.012 : crossBrain ? 0.006 : kind === "contains" ? 0.019 : 0.016;
      const magnitude = (distance - desiredLength) * strength;
      const fx = (dx / distance) * magnitude;
      const fy = (dy / distance) * magnitude;
      if (!sourcePosition.lockedX) forceX[source] += fx;
      if (!sourcePosition.lockedY) forceY[source] += fy;
      if (!targetPosition.lockedX) forceX[target] -= fx;
      if (!targetPosition.lockedY) forceY[target] -= fy;
    }

    const temperature = 0.38 * (1 - step / iterations) + 0.06;
    positions.forEach((position, index) => {
      const anchorStrength = orderedNodes[index].kind === "brain" ? 0.05 : 0.028;
      if (!position.lockedX) {
        forceX[index] += (position.clusterX - position.x) * anchorStrength;
        if (position.x < 8) forceX[index] += (8 - position.x) * 0.3;
        if (position.x > 92) forceX[index] -= (position.x - 92) * 0.3;
        const cappedForce = Math.max(-1.25, Math.min(1.25, forceX[index]));
        position.vx = Math.max(-0.72, Math.min(0.72, (position.vx + cappedForce * temperature) * 0.7));
        position.x = clampGraphCoordinate(position.x + position.vx);
      }
      if (!position.lockedY) {
        forceY[index] += (position.clusterY - position.y) * anchorStrength;
        if (position.y < 8) forceY[index] += (8 - position.y) * 0.3;
        if (position.y > 92) forceY[index] -= (position.y - 92) * 0.3;
        const cappedForce = Math.max(-1.25, Math.min(1.25, forceY[index]));
        position.vy = Math.max(-0.72, Math.min(0.72, (position.vy + cappedForce * temperature) * 0.7));
        position.y = clampGraphCoordinate(position.y + position.vy);
      }
    });
  }

  return orderedNodes.map((node, index) => ({
    ...node,
    x: Number(positions[index].x.toFixed(3)),
    y: Number(positions[index].y.toFixed(3)),
  }));
}

function normalizeGraph(value: unknown): GraphSnapshot | null {
  const record = asRecord(value);
  if (!record || !Array.isArray(record.nodes) || !Array.isArray(record.edges)) return null;
  const nodes = record.nodes.flatMap((item): GraphNode[] => {
    const node = asRecord(item);
    if (!node || typeof node.id !== "string") return [];
    const kind = typeof node.kind === "string" ? node.kind : "note";
    const clusterId = typeof node.clusterId === "string" ? node.clusterId : undefined;
    const brainId = typeof node.brainId === "string"
      ? node.brainId
      : kind === "kairos"
        ? "kairos"
        : clusterId?.replace(/^brain:/, "") ?? "unassigned";
    return [{
      id: node.id,
      label: typeof node.label === "string" ? node.label : node.id,
      brainId,
      clusterId,
      relativePath: typeof node.relativePath === "string" ? node.relativePath : undefined,
      kind,
      x: typeof node.x === "number" && Number.isFinite(node.x) ? node.x : undefined,
      y: typeof node.y === "number" && Number.isFinite(node.y) ? node.y : undefined,
      protected: Boolean(node.protected),
    }];
  });
  const edges = record.edges.flatMap((item): GraphEdge[] => {
    const edge = asRecord(item);
    if (!edge || typeof edge.source !== "string" || typeof edge.target !== "string") return [];
    return [{
      id: typeof edge.id === "string" ? edge.id : undefined,
      source: edge.source,
      target: edge.target,
      kind: typeof edge.kind === "string" ? edge.kind : undefined,
    }];
  });
  const needsLayout = nodes.some((node) => node.x === undefined || node.y === undefined);
  return nodes.length > 0 ? {
    nodes: needsLayout ? layoutGraph(nodes, edges) : nodes,
    edges,
    indexedAt: typeof record.indexedAt === "string" ? record.indexedAt : undefined,
    stale: Boolean(record.stale),
  } : null;
}

function CitationChip({ citation }: { citation: Citation }) {
  const visual = brainVisual(citation.brainId);
  return (
    <span
      className="citation-chip"
      style={{ "--chip-color": visual.color } as CSSProperties}
      title={citation.relativePath}
    >
      <img src={visual.icon} alt="" aria-hidden="true" />
      <span>{visual.label}</span>
      <span className="citation-chip__detail">{compactPath(citation.relativePath)}</span>
    </span>
  );
}

function KairosLoader() {
  return (
    <svg className="kairos-loader" viewBox="0 0 64 64" role="status" aria-label="Kairos is routing">
      <path className="kairos-loader__ring" d="M46 13a23 23 0 1 0 0 38" fill="none" stroke="currentColor" strokeWidth="4" strokeLinecap="round" />
      <g fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
        <path d="M25 19v9l7 4 9-8" />
        <path d="m32 32 9 8" />
        <path d="M25 36v9" />
      </g>
      <circle className="kairos-loader__core" cx="32" cy="32" r="4" fill="#76E4FF" />
      <circle className="kairos-loader__moment" cx="50" cy="32" r="4" fill="#FFC766" />
    </svg>
  );
}

function StatusPill({ tone = "neutral", children }: { tone?: "neutral" | "success" | "warning" | "danger" | "local"; children: string }) {
  return <span className={`status-pill status-pill--${tone}`}>{children}</span>;
}

function RouteChips({ routes }: { routes?: Array<{ id: string; name: string }> }) {
  if (!routes?.length) return null;
  return (
    <div className="route-chip-row" aria-label="Routed brains">
      {routes.map((route) => {
        const visual = brainVisual(route.id);
        return (
          <span className="route-chip" key={route.id} style={{ "--route-color": visual.color } as CSSProperties}>
            <span className="route-chip__dot" />
            {route.name}
          </span>
        );
      })}
    </div>
  );
}

export default function App() {
  const nativeRuntime = hasNativeBridge();
  const [status, setStatus] = useState<AppStatus | null>(() => nativeRuntime ? null : browserPreviewStatus);
  const [localSetup, setLocalSetup] = useState<LocalSetupStatus | null>(() => nativeRuntime ? null : browserPreviewSetup);
  const [view, setView] = useState<ViewId>("chat");
  const [settingsAnchor, setSettingsAnchor] = useState<SettingsAnchor>("top");
  const [surfaceMode, setSurfaceMode] = useState<SurfaceMode>("compact");
  const expanded = surfaceMode === "cockpit";
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [capabilityNotice, setCapabilityNotice] = useState<string | null>(null);
  const [briefResult, setBriefResult] = useState<LocalBrief | null>(null);
  const [chatMessages, setChatMessages] = useState<ChatMessage[]>(defaultChatMessages);
  const [streamingAssistant, setStreamingAssistant] = useState<StreamingAssistant | null>(null);
  const [composer, setComposer] = useState("");
  const composerRef = useRef<HTMLTextAreaElement | null>(null);
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [attachmentNotice, setAttachmentNotice] = useState<string | null>(null);
  const [providerId, setProviderId] = useState<ProviderId>("ollama");
  const [kairosConsentMode, setKairosConsentMode] = useState<KairosConsentMode>("ask");
  const [scopedExecutionGrant, setScopedExecutionGrant] = useState<ScopedExecutionGrant | null>(null);
  const [brainOverride, setBrainOverride] = useState("auto");
  const [chatPreview, setChatPreview] = useState<ChatPreview | null>(null);
  const [chatSessionId, setChatSessionId] = useState<string | null>(null);
  const [chatSessions, setChatSessions] = useState<ChatSessionSummary[]>([]);
  const [chatSearch, setChatSearch] = useState("");
  const [historyOpen, setHistoryOpen] = useState(false);
  const [providerModel, setProviderModel] = useState("");
  const [providerApiKey, setProviderApiKey] = useState("");
  const [providerSettingsBusy, setProviderSettingsBusy] = useState(false);
  const [providerSettingsFeedback, setProviderSettingsFeedback] = useState<string | null>(null);
  const [hardwareProfile, setHardwareProfile] = useState<HardwareProfile>("auto");
  const [hardwareProfileBusy, setHardwareProfileBusy] = useState(false);
  const [memoryBudget, setMemoryBudget] = useState<MemoryBudget>("auto");
  const [customMemoryBudget, setCustomMemoryBudget] = useState("48");
  const [memoryBudgetBusy, setMemoryBudgetBusy] = useState(false);
  const [modelOperation, setModelOperation] = useState<{ kind: "pull" | "test"; model: string } | null>(null);
  const [pullProgress, setPullProgress] = useState<OllamaPullProgress | null>(null);
  const [failedPullModel, setFailedPullModel] = useState<string | null>(null);
  const [setupFeedback, setSetupFeedback] = useState<string | null>(null);
  const [showAllModels, setShowAllModels] = useState(false);
  const [brains, setBrains] = useState<BrainRecord[]>([]);
  const [policyEditor, setPolicyEditor] = useState<BrainPolicyDraft | null>(null);
  const [policyBusy, setPolicyBusy] = useState(false);
  const [policyFeedback, setPolicyFeedback] = useState<string | null>(null);
  const [graph, setGraph] = useState<GraphSnapshot>(fallbackGraph);
  const [graphBusy, setGraphBusy] = useState(false);
  const [graphFilter, setGraphFilter] = useState("all");
  const [graphSearch, setGraphSearch] = useState("");
  const [selectedGraphNode, setSelectedGraphNode] = useState<string | null>(null);
  const [hoveredGraphNode, setHoveredGraphNode] = useState<string | null>(null);
  const [graphCamera, setGraphCamera] = useState<GraphCamera>(graphCameraFit);
  const [previousGraphCamera, setPreviousGraphCamera] = useState<GraphCamera | null>(null);
  const [graphViewportSize, setGraphViewportSize] = useState({ width: 1, height: 1 });
  const graphCanvasRef = useRef<HTMLDivElement | null>(null);
  const graphDragRef = useRef<{ pointerId: number; startX: number; startY: number; camera: GraphCamera } | null>(null);
  const [addBrain, setAddBrain] = useState<{
    selectionToken: string;
    displayPath?: string;
    name: string;
    role: string;
    routingHints: string;
    egressPolicy: string;
    writePolicy: string;
    graphEnabled: boolean;
    noteCount?: number;
    routerCandidates: string[];
    contextCandidates: string[];
  } | null>(null);
  const [preferences, setPreferences] = useState({
    shortcut: "Option+Space",
    summonTarget: "compact" as "compact" | "last-page",
    launchAtLogin: false,
    closeToHide: true,
    keepAboveOtherWindows: false,
    contextWindowTokens: 32_768,
  });
  const [preferencesFeedback, setPreferencesFeedback] = useState<string | null>(null);
  const [writeDraft, setWriteDraft] = useState({ brainId: "", kind: "create" as "create" | "edit", relativePath: "Notes/Kairos Capture.md", markdown: "" });
  const [writeProposal, setWriteProposal] = useState<NoteWriteProposal | null>(null);
  const [writeBusy, setWriteBusy] = useState(false);
  const [writeFeedback, setWriteFeedback] = useState<string | null>(null);

  const activeProvider = providers.find((provider) => provider.id === providerId) ?? providers[0];
  const activeKairosConsentMode = kairosConsentModes.find((mode) => mode.id === kairosConsentMode) ?? kairosConsentModes[0];
  const activeModel = status?.model.resolvedModel ?? status?.model.selectedModel ?? "Checking Ollama";
  const ollamaRunning = localSetup?.running ?? status?.model.running ?? false;
  const ollamaInstalled = localSetup?.ollamaInstalled ?? ollamaRunning;
  const selectedModelInstalled = localSetup?.selectedModelInstalled ?? status?.model.selectedModelInstalled ?? false;
  const detectedMemoryGb = localSetup?.detectedMemoryGb ?? (nativeRuntime ? 0 : 48);
  const effectiveHardwareProfile = localSetup?.effectiveHardwareProfile ?? localSetup?.detectedHardwareProfile ?? "apple_unified";
  const hardwareProfileLabel = localSetup?.hardwareProfileLabel
    ?? (effectiveHardwareProfile === "nvidia_vram" ? "NVIDIA GPU" : effectiveHardwareProfile === "cpu_only" ? "CPU-only" : "Apple Silicon");
  const primaryFitLimitGb = localSetup?.primaryFitLimitGb ?? detectedMemoryGb;
  const primaryFitLimitLabel = localSetup?.primaryFitLimitLabel
    ?? (effectiveHardwareProfile === "nvidia_vram" ? "NVIDIA VRAM" : effectiveHardwareProfile === "cpu_only" ? "System memory" : "Apple unified memory");
  const hardwarePlanningOverride = localSetup?.hardwarePlanningOverride ?? hardwareProfile !== "auto";
  const availableDiskGb = localSetup?.availableDiskGb ?? (nativeRuntime ? 0 : 532);
  const customMemoryBudgetValue = Math.max(1, Math.min(192, Number(customMemoryBudget) || primaryFitLimitGb || detectedMemoryGb || 1));
  const effectiveMemoryBudget = memoryBudget === "auto"
    ? (localSetup?.effectiveMemoryBudgetGb ?? (detectedMemoryGb || 48))
    : memoryBudget === "custom"
      ? customMemoryBudgetValue
      : memoryBudget;
  const memoryBudgetPending = memoryBudget === "custom"
    && (localSetup?.memoryBudgetMode !== "custom" || localSetup?.memoryBudgetGb !== customMemoryBudgetValue);
  const selectedModelFit = localSetup?.selectedModelFit ?? status?.model.selectedModelFit;
  const modelReady = Boolean(status?.initialized && ollamaRunning && selectedModelInstalled);
  const chatCanSend = Boolean(composer.trim()) && !busy && (activeProvider.external || modelReady);
  const composerProviderDetail = !activeProvider.external && !modelReady
    ? (localSetup?.setupMessage ?? status?.model.setupMessage ?? "Select and download a ready local model before sending.")
    : activeProvider.detail;
  const selectedBrain = brains.find((brain) => brain.id === brainOverride);
  const writableBrains = brains.filter(canConfirmWrites);
  const graphNode = graph.nodes.find((node) => node.id === selectedGraphNode);
  const catalogModels = useMemo(() => {
    const responseModels = [
      ...(localSetup?.advancedModels ?? []),
      ...(localSetup?.models ?? []),
      ...(localSetup?.recommendedModels ?? []),
    ];
    const sourceModels: LocalSetupModel[] = responseModels.length
      ? responseModels
      : (status?.model.choices ?? []).map((choice) => ({
        id: choice.id,
        label: choice.label,
        role: choice.role,
        installed: isInstalledModel(choice.id, status?.model.installedModels ?? []),
        downloadSize: modelCatalog[choice.id]?.downloadSize,
        recommendedContext: modelCatalog[choice.id]?.context,
        capabilities: [],
      }));
    const modelsById = new Map<string, LocalSetupModel>();
    sourceModels.forEach((model) => {
      const key = normalizedModelId(model.id);
      const existing = modelsById.get(key);
      modelsById.set(key, existing ? { ...existing, ...model } : model);
    });
    return [...modelsById.values()];
  }, [localSetup?.advancedModels, localSetup?.models, localSetup?.recommendedModels, status?.model.choices, status?.model.installedModels]);
  const recommendedModelCards = useMemo(() => {
    const hasRecommendationResponse = localSetup?.recommendedModels !== undefined;
    const responseModels = localSetup?.recommendedModels ?? catalogModels;
    const selectedModel = status?.model.selectedModel ?? "";
    const seen = new Set<string>();
    return responseModels
      .filter((model) => hasRecommendationResponse
        ? model.defaultRecommended === true && model.advancedOnly !== true && model.fit?.fit === "recommended"
        : model.advancedOnly !== true && model.fit?.fit === "recommended" && !model.fit.requiresTest)
      .sort((left, right) => (left.recommendationRank ?? Number.MAX_SAFE_INTEGER) - (right.recommendationRank ?? Number.MAX_SAFE_INTEGER))
      .filter((model) => {
        const key = normalizedModelId(model.id);
        if (seen.has(key) || sameModelId(model.id, selectedModel)) return false;
        seen.add(key);
        return true;
      })
      .slice(0, 4);
  }, [catalogModels, localSetup?.recommendedModels, status?.model.selectedModel]);
  const selectedModelCard = useMemo<LocalSetupModel | null>(() => {
    const selectedModel = status?.model.selectedModel;
    if (!selectedModel) return null;
    const matchedModel = catalogModels.find((model) => sameModelId(model.id, selectedModel));
    return {
      id: selectedModel,
      label: matchedModel?.label ?? selectedModel,
      role: matchedModel?.role,
      installed: selectedModelInstalled,
      downloadSize: matchedModel?.downloadSize,
      recommendedContext: matchedModel?.recommendedContext,
      fit: selectedModelFit ?? matchedModel?.fit,
      capabilities: matchedModel?.capabilities ?? [],
      variant: matchedModel?.variant,
      whyRecommended: matchedModel?.whyRecommended,
      verifiedAt32k: matchedModel?.verifiedAt32k,
    };
  }, [catalogModels, selectedModelFit, selectedModelInstalled, status?.model.selectedModel]);
  const advancedModelCards = useMemo(() => {
    const recommendedIds = new Set(recommendedModelCards.map((model) => normalizedModelId(model.id)));
    const selectedModel = status?.model.selectedModel ?? "";
    const responseModels = localSetup?.advancedModels ?? catalogModels;
    const seen = new Set<string>();
    return responseModels
      .filter((model) => {
        const key = normalizedModelId(model.id);
        if (seen.has(key) || recommendedIds.has(key) || sameModelId(model.id, selectedModel)) return false;
        seen.add(key);
        return true;
      })
      .sort((left, right) => {
        const fitDifference = modelFitRank(left.fit) - modelFitRank(right.fit);
        if (fitDifference) return fitDifference;
        return (left.label ?? left.id).localeCompare(right.label ?? right.id);
      });
  }, [catalogModels, localSetup?.advancedModels, recommendedModelCards, status?.model.selectedModel]);
  const installedOllamaChoices = useMemo(() => {
    const choices = status?.model.choices ?? [];
    const installedModels = status?.model.installedModels ?? [];
    const uniqueChoices = new Map<string, OllamaModelChoice>();
    installedModels.forEach((installedModel) => {
      const matchingChoice = choices.find((choice) => sameModelId(choice.id, installedModel));
      const id = matchingChoice?.id ?? installedModel;
      const key = normalizedModelId(id);
      if (!uniqueChoices.has(key)) {
        uniqueChoices.set(key, {
          id,
          label: matchingChoice?.label ?? installedModel,
          role: matchingChoice?.role ?? "Installed Ollama model",
        });
      }
    });
    return [...uniqueChoices.values()].sort((left, right) => {
      const selectedDifference = Number(sameModelId(right.id, status?.model.selectedModel ?? "")) - Number(sameModelId(left.id, status?.model.selectedModel ?? ""));
      return selectedDifference || left.label.localeCompare(right.label);
    });
  }, [status?.model.choices, status?.model.installedModels, status?.model.selectedModel]);
  const selectedInstalledOllamaModel = installedOllamaChoices.find((choice) => sameModelId(choice.id, status?.model.selectedModel ?? ""))?.id ?? "";

  const routeIcon = error ? routeError : briefResult
    ? briefResult.context.route.brains.length > 1 ? routeMulti : routeOne
    : routeIdle;

  const routeLabel = busy
    ? "Routing"
    : error
      ? "Needs attention"
      : "Ready to route";

  const callFeature = async <T,>(command: string, args?: Record<string, unknown>, unavailableMessage?: string): Promise<T | null> => {
    if (!nativeRuntime) {
      setCapabilityNotice("This is a browser preview. Open Kairos for local vault and model actions.");
      return null;
    }
    try {
      return await invoke<T>(command, args);
    } catch (reason) {
      setCapabilityNotice(
        reason instanceof Error && reason.message
          ? reason.message
          : unavailableMessage ?? "This control needs the matching native Kairos update.",
      );
      return null;
    }
  };

  const refreshStatus = async () => {
    if (!nativeRuntime) {
      setStatus(browserPreviewStatus);
      setLocalSetup(browserPreviewSetup);
      return;
    }
    try {
      const nextStatus = await invoke<AppStatus>("app_status");
      setStatus(nextStatus);
      if (nextStatus.app) {
        setPreferences((current) => ({
          ...current,
          shortcut: nextStatus.app?.summonShortcut?.replace("Alt", "Option") ?? current.shortcut,
          summonTarget: "compact",
          launchAtLogin: nextStatus.app?.launchAtLogin ?? current.launchAtLogin,
          closeToHide: nextStatus.app?.closeToHide ?? current.closeToHide,
          keepAboveOtherWindows: nextStatus.app?.keepAboveOtherWindows ?? current.keepAboveOtherWindows,
          contextWindowTokens: nextStatus.model.contextWindowTokens ?? current.contextWindowTokens,
        }));
      }
      setError(null);
    } catch (reason) {
      setError(String(reason));
    }
    try {
      const nextSetup = await invoke<LocalSetupStatus>("local_setup_status");
      setLocalSetup(nextSetup);
    } catch {
      // Older native builds keep the setup view informative from app_status.
    }
  };

  const refreshBrains = async () => {
    const result = await callFeature<unknown>(
      "list_brains",
      undefined,
      "Brain management arrives with the next native Kairos update.",
    );
    const nextBrains = normalizeBrains(result);
    if (nextBrains) setBrains(nextBrains);
  };

  const refreshGraph = async () => {
    setGraphBusy(true);
    const result = await callFeature<unknown>(
      "graph_snapshot",
      undefined,
      "The local metadata graph is not available in this Kairos build yet.",
    );
    const nextGraph = normalizeGraph(result);
    if (nextGraph) setGraph(nextGraph);
    setGraphBusy(false);
  };

  const revealGraphNode = async (nodeId: string) => {
    if (!nativeRuntime) {
      setCapabilityNotice("Reveal in Finder is available in the Kairos desktop app.");
      return;
    }
    try {
      await invoke("reveal_graph_node", { nodeId });
      setCapabilityNotice("Opened the selected graph source in Finder.");
    } catch (reason) {
      setCapabilityNotice(reason instanceof Error ? reason.message : "Kairos could not reveal that graph item.");
    }
  };

  useEffect(() => {
    void refreshStatus();
  }, []);

  useEffect(() => {
    const mode = localSetup?.memoryBudgetMode;
    if (!mode) return;
    if (mode === "auto") {
      setMemoryBudget("auto");
      return;
    }
    if (mode === "custom") {
      setMemoryBudget("custom");
      if (typeof localSetup.memoryBudgetGb === "number") {
        setCustomMemoryBudget(String(localSetup.memoryBudgetGb));
      }
      return;
    }
    if (typeof localSetup.memoryBudgetGb === "number") {
      setMemoryBudget(localSetup.memoryBudgetGb as MemoryBudget);
    }
  }, [localSetup?.memoryBudgetMode, localSetup?.memoryBudgetGb]);

  useEffect(() => {
    if (localSetup?.selectedHardwareProfile) {
      setHardwareProfile(localSetup.selectedHardwareProfile);
    }
  }, [localSetup?.selectedHardwareProfile]);

  useEffect(() => {
    if (!nativeRuntime) return;
    let dispose: (() => void) | undefined;
    void listen<"compact_chat" | "last_surface">("kairos://summon", () => {
      setView("chat");
      setSurfaceMode("compact");
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, [nativeRuntime]);

  useEffect(() => {
    if (!nativeRuntime) return;
    let dispose: (() => void) | undefined;
    void listen<unknown>("kairos-chat-delta", (event) => {
      const payload = asRecord(event.payload);
      const turnId = typeof payload?.turnId === "string" ? payload.turnId : undefined;
      const delta = typeof payload?.delta === "string" ? payload.delta : undefined;
      if (!turnId || !delta) return;
      setStreamingAssistant((current) => current?.turnId === turnId
        ? { ...current, body: `${current.body}${delta}` }
        : current);
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, [nativeRuntime]);

  useEffect(() => {
    if (!nativeRuntime) return;
    let dispose: (() => void) | undefined;
    void listen<unknown>("ollama-pull-progress", (event) => {
      const payload = asRecord(event.payload);
      const model = typeof payload?.model === "string" ? payload.model : undefined;
      if (!model) return;
      const status = typeof payload?.status === "string" ? payload.status : "Downloading model…";
      const completed = typeof payload?.completed === "number" ? payload.completed : undefined;
      const total = typeof payload?.total === "number" ? payload.total : undefined;
      const percent = typeof payload?.percent === "number"
        ? Math.max(0, Math.min(100, payload.percent))
        : completed !== undefined && total && total > 0
          ? Math.max(0, Math.min(100, (completed / total) * 100))
          : undefined;
      const normalized = status.toLowerCase();
      const failed = Boolean(payload?.error) || ["failed", "error"].some((word) => normalized.includes(word));
      const finished = failed || Boolean(payload?.done) || ["complete", "success", "cancelled", "canceled"].some((word) => normalized.includes(word));
      if (finished) {
        setPullProgress(null);
        setModelOperation((current) => current?.kind === "pull" && current.model === model ? null : current);
        if (failed) {
          setFailedPullModel(model);
          setSetupFeedback(`${model} did not finish downloading. Check Ollama, then retry.`);
        }
        return;
      }
      setPullProgress({ model, status, completed, total, percent });
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, [nativeRuntime]);

  useEffect(() => {
    if (!nativeRuntime) return;
    void invoke("set_surface_mode", { expanded }).catch(() => {
      // Older native builds retain their fixed compact window size.
    });
  }, [expanded, nativeRuntime]);

  useEffect(() => {
    const animationFrame = window.requestAnimationFrame(() => composerRef.current?.focus());
    return () => window.cancelAnimationFrame(animationFrame);
  }, [surfaceMode]);

  useEffect(() => {
    if (!expanded || !graphCanvasRef.current) return;
    const canvas = graphCanvasRef.current;
    const updateSize = () => {
      const bounds = canvas.getBoundingClientRect();
      setGraphViewportSize({
        width: Math.max(1, bounds.width),
        height: Math.max(1, bounds.height),
      });
    };
    updateSize();
    const observer = new ResizeObserver(updateSize);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [expanded]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !expanded) return;
      if (selectedGraphNode) {
        setSelectedGraphNode(null);
        if (previousGraphCamera) {
          setGraphCamera(previousGraphCamera);
          setPreviousGraphCamera(null);
        }
        return;
      }
      if (view !== "chat" && view !== "map") {
        setView("chat");
        return;
      }
      setView("chat");
      setSurfaceMode("compact");
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [expanded, previousGraphCamera, selectedGraphNode, view]);

  const refreshChatSessions = async (query = chatSearch) => {
    if (!nativeRuntime) return;
    try {
      const sessions = await invoke<ChatSessionSummary[]>("list_chat_sessions", { query: query.trim() || undefined });
      setChatSessions(sessions);
    } catch (reason) {
      setCapabilityNotice(reason instanceof Error ? reason.message : "Kairos could not load chat history.");
    }
  };

  const startNewChat = async () => {
    setHistoryOpen(false);
    if (!nativeRuntime) {
      setChatSessionId(null);
      setChatMessages(defaultChatMessages());
      return;
    }
    try {
      const session = await invoke<StoredChatSession>("create_chat_session");
      setChatSessionId(session.id);
      setChatMessages(defaultChatMessages());
      await refreshChatSessions("");
    } catch (reason) {
      setCapabilityNotice(reason instanceof Error ? reason.message : "Kairos could not start a new chat.");
    }
  };

  const loadChatSession = async (sessionId: string) => {
    if (!nativeRuntime) return;
    try {
      const session = await invoke<StoredChatSession>("load_chat_session", { sessionId });
      setChatSessionId(session.id);
      setHistoryOpen(false);
      setChatMessages(session.messages.length ? session.messages.map((message) => ({
        id: message.id,
        role: message.role,
        body: message.content,
        createdAt: message.createdAt,
        provider: message.providerLabel,
        routes: (message.routeBrainIds ?? []).map((id) => ({ id, name: brainVisual(id).label })),
        citations: (message.sourceIds ?? []).map((id) => ({ id, brainId: id.split(":")[0] ?? "", relativePath: id.split(":").slice(1).join(":") })),
      })) : defaultChatMessages());
    } catch (reason) {
      setCapabilityNotice(reason instanceof Error ? reason.message : "Kairos could not open that chat.");
    }
  };

  const deleteChatSession = async (sessionId: string) => {
    if (!nativeRuntime || !window.confirm("Delete this local Kairos chat? This cannot delete any brain notes.")) return;
    try {
      await invoke("delete_chat_session", { sessionId });
      if (chatSessionId === sessionId) {
        setChatSessionId(null);
        setChatMessages(defaultChatMessages());
      }
      await refreshChatSessions("");
    } catch (reason) {
      setCapabilityNotice(reason instanceof Error ? reason.message : "Kairos could not delete that chat.");
    }
  };

  const prepareChatLog = () => {
    const brain = selectedBrain && canConfirmWrites(selectedBrain) ? selectedBrain : writableBrains[0];
    if (!brain) {
      setCapabilityNotice("Enable Confirm every write on a brain before saving a chat as a note.");
      return;
    }
    const day = new Date().toISOString().slice(0, 10);
    const transcript = chatMessages
      .filter((message) => !message.notice)
      .map((message) => `## ${message.role === "user" ? "You" : "Kairos"}\n\n${message.body}`)
      .join("\n\n");
    if (!transcript.trim()) {
      setCapabilityNotice("Send at least one message before saving this chat to a brain.");
      return;
    }
    setWriteProposal(null);
    setWriteFeedback("Chat prepared as a local Markdown draft. Review the diff before anything is written.");
    setWriteDraft({
      brainId: brain.id,
      kind: "create",
      relativePath: `01_Daily/${day} Kairos Chat.md`,
      markdown: `---\ntype: kairos_chat\ncreated: ${new Date().toISOString()}\n---\n\n# Kairos chat\n\n${transcript}\n`,
    });
    setSurfaceMode("cockpit");
    setSettingsAnchor("writes");
    setView("settings");
  };

  useEffect(() => {
    if (nativeRuntime) void refreshChatSessions("");
  }, [nativeRuntime]);

  useEffect(() => {
    if (view === "map") void refreshGraph();
    if (view === "settings") void refreshBrains();
  }, [view]);

  useEffect(() => {
    if (view !== "settings" || settingsAnchor === "top") return;
    const targetId = settingsAnchor === "brains"
      ? "brain-registry-settings"
      : settingsAnchor === "privacy"
        ? "provider-access-settings"
        : "confirmed-write-settings";
    const animationFrame = window.requestAnimationFrame(() => {
      document.getElementById(targetId)?.scrollIntoView({ behavior: "smooth", block: "start" });
    });
    return () => window.cancelAnimationFrame(animationFrame);
  }, [settingsAnchor, view]);

  useEffect(() => {
    if (expanded) {
      void refreshGraph();
      void refreshBrains();
    }
  }, [expanded]);

  const selectModel = async (model: string) => {
    if (!nativeRuntime) {
      setCapabilityNotice("This is a browser preview. Choose and save local models in the Kairos desktop app.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setStatus(await invoke<AppStatus>("set_selected_model", { model }));
      setBriefResult(null);
      await refreshStatus();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const runPulse = async () => {
    if (activeProvider.external) {
      setView("chat");
      setSurfaceMode("cockpit");
      setComposer(briefQuestion);
      await previewExternalTurn(briefQuestion);
      return;
    }
    if (!nativeRuntime) {
      setCapabilityNotice("What Next uses the local vault only in the Kairos desktop app.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const current = await invoke<AppStatus>("app_status");
      setStatus(current);
      if (!current.model.running || !current.model.selectedModelInstalled) {
        setBriefResult(null);
        setError(current.model.setupMessage ?? "The selected local model is not ready yet.");
        return;
      }
      setBriefResult(await invoke<LocalBrief>("brief_with_ollama"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const previewExternalTurn = async (message: string) => {
    const history = chatMessages
      .filter((chat) => chat.role === "user" || chat.role === "assistant")
      .slice(-8)
      .map((chat) => ({ role: chat.role, content: chat.body }));
    const result = await callFeature<unknown>(
      "chat_preview",
      {
        message,
        brainOverride: brainOverride === "auto" ? undefined : brainOverride,
        providerId,
        history,
        attachmentIds: attachments.map((attachment) => attachment.id),
      },
      "External-provider preview is not available in this native build yet. Nothing was sent.",
    );
    const raw = asRecord(result);
    const route = asRecord(raw?.route);
    const routeBrains = Array.isArray(route?.brains) ? route?.brains : [];
    const routes = routeBrains.flatMap((item): Array<{ id: string; name: string }> => {
      const itemRecord = asRecord(item);
      return itemRecord && typeof itemRecord.id === "string" && typeof itemRecord.name === "string"
        ? [{ id: itemRecord.id, name: itemRecord.name }]
        : [];
    });
    const context = asRecord(raw?.context);
    const sourceItems = Array.isArray(context?.sources) ? context?.sources : [];
    const citations = sourceItems.flatMap((item): Citation[] => {
      const itemRecord = asRecord(item);
      const source = asRecord(itemRecord?.source);
      return source && typeof source.id === "string" && typeof source.brainId === "string" && typeof source.relativePath === "string"
        ? [sourceCitation({
          id: source.id,
          brainId: source.brainId,
          relativePath: source.relativePath,
          modifiedAt: typeof source.modifiedAt === "string" ? source.modifiedAt : undefined,
        })]
        : [];
    });
    setChatPreview({
      previewToken: typeof raw?.previewToken === "string" ? raw.previewToken : undefined,
      message,
      providerId,
      providerLabel: activeProvider.label,
      routes: routes.length ? routes : selectedBrain ? [{ id: selectedBrain.id, name: selectedBrain.name }] : [],
      citations: citations.slice(0, 3),
      attachmentNames: Array.isArray(raw?.attachmentNames)
        ? raw.attachmentNames.filter((name): name is string => typeof name === "string")
        : attachments.map((attachment) => attachment.name),
      outgoingSummary: typeof raw?.outgoingSummary === "string"
        ? raw.outgoingSummary
        : message,
    });
  };

  const messageFromResult = (result: unknown, fallback: string): ChatMessage => {
    const raw = asRecord(result);
    const resultContext = asRecord(raw?.context);
    const resultRoute = asRecord(resultContext?.route);
    const routeItems = Array.isArray(resultRoute?.brains) ? resultRoute?.brains : [];
    const routes = routeItems.flatMap((item): Array<{ id: string; name: string }> => {
      const route = asRecord(item);
      return route && typeof route.id === "string" && typeof route.name === "string"
        ? [{ id: route.id, name: route.name }]
        : [];
    });
    const sources = Array.isArray(resultContext?.sources) ? resultContext?.sources : [];
    const citations = sources.flatMap((item): Citation[] => {
      const sourceRecord = asRecord(asRecord(item)?.source);
      return sourceRecord && typeof sourceRecord.id === "string" && typeof sourceRecord.brainId === "string" && typeof sourceRecord.relativePath === "string"
        ? [sourceCitation({
          id: sourceRecord.id,
          brainId: sourceRecord.brainId,
          relativePath: sourceRecord.relativePath,
          modifiedAt: typeof sourceRecord.modifiedAt === "string" ? sourceRecord.modifiedAt : undefined,
        })]
        : [];
    });
    const answer = raw?.answer;
    const answerRecord = asRecord(answer);
    const body = typeof answer === "string"
      ? answer
      : typeof raw?.message === "string"
        ? raw.message
        : answerRecord && typeof answerRecord.action === "string"
          ? `${answerRecord.action}\n\n${typeof answerRecord.why === "string" ? answerRecord.why : ""}`.trim()
          : fallback;
    return {
      id: makeId("assistant"),
      role: "assistant",
      body,
      createdAt: new Date().toISOString(),
      routes,
      citations: citations.slice(0, 3),
      provider: activeProvider.label,
      notice: body === fallback,
    };
  };

  const submitChat = async (confirmedExternal = false) => {
    const message = chatPreview?.message ?? composer.trim();
    if (!message || busy) return;
    if (!activeProvider.external && !modelReady) {
      setError(localSetup?.setupMessage ?? status?.model.setupMessage ?? "The selected local model is not ready. Kairos did not switch models.");
      return;
    }
    if (activeProvider.external && !confirmedExternal) {
      await previewExternalTurn(message);
      return;
    }
    let effectiveSessionId = chatSessionId;
    let executionGrant: string | undefined;
    if (kairosConsentMode === "full_kairos") {
      if (!nativeRuntime) {
        setCapabilityNotice("Scoped full access is available in the native Kairos app.");
        return;
      }
      if (brainOverride === "auto") {
        setError("Choose one brain in Route before granting scoped full access.");
        return;
      }
      try {
        if (!effectiveSessionId) {
          const session = await invoke<StoredChatSession>("create_chat_session");
          effectiveSessionId = session.id;
          setChatSessionId(session.id);
        }
        const reusable = scopedExecutionGrant
          && scopedExecutionGrant.sessionId === effectiveSessionId
          && scopedExecutionGrant.providerId === providerId
          && scopedExecutionGrant.brainId === brainOverride
          && new Date(scopedExecutionGrant.expiresAt).valueOf() > Date.now();
        if (reusable) {
          executionGrant = scopedExecutionGrant.token;
        } else {
          const ok = window.confirm(`Grant Kairos scoped access to ${selectedBrain?.name ?? brainOverride} for this chat for 30 minutes? This never grants shell, arbitrary filesystem, deletion, private-note access, or unreviewed external sending.`);
          if (!ok) return;
          const grant = await invoke<{ token: string; brainId: string; expiresAt: string }>("grant_scoped_execution", {
            sessionId: effectiveSessionId,
            providerId,
            brainId: brainOverride,
          });
          executionGrant = grant.token;
          setScopedExecutionGrant({ ...grant, providerId, sessionId: effectiveSessionId });
        }
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : "Kairos could not create the scoped-execution grant.");
        return;
      }
    }
    const turnId = activeProvider.id === "ollama" ? makeId("turn") : undefined;
    setBusy(true);
    setError(null);
    if (turnId) {
      setStreamingAssistant({ turnId, body: "", createdAt: new Date().toISOString() });
    }
    const userMessage: ChatMessage = {
      id: makeId("user"),
      role: "user",
      body: message,
      createdAt: new Date().toISOString(),
      routes: selectedBrain ? [{ id: selectedBrain.id, name: selectedBrain.name }] : undefined,
    };
    setChatMessages((messages) => [...messages, userMessage]);
    setComposer("");
    setChatPreview(null);
    try {
      const history = chatMessages
        .filter((chat) => chat.role === "user" || chat.role === "assistant")
        .slice(-8)
        .map((chat) => ({ role: chat.role, content: chat.body }));
      const result = await callFeature<unknown>(
      "chat_with_provider",
      {
          request: {
            message,
            brainOverride: brainOverride === "auto" ? undefined : brainOverride,
            providerId,
            confirmedExternal,
            previewToken: chatPreview?.previewToken,
            history,
            sessionId: effectiveSessionId,
            attachmentIds: attachments.map((attachment) => attachment.id),
            turnId,
            executionGrant,
          },
        },
        "Chat generation is not available in this native build yet. Your text has not been sent to another provider.",
      );
      const fallback = activeProvider.external
        ? "This cloud handoff was not sent because the native confirmation bridge is not available yet."
        : "Local chat is waiting for the native chat engine. Try What Next for the current local brief.";
      const resultRecord = asRecord(result);
      if (typeof resultRecord?.sessionId === "string") {
        setChatSessionId(resultRecord.sessionId);
        setAttachments([]);
        await refreshChatSessions("");
      }
      setChatMessages((messages) => [...messages, messageFromResult(result, fallback)]);
    } finally {
      if (turnId) {
        setStreamingAssistant((current) => current?.turnId === turnId ? null : current);
      }
      setBusy(false);
    }
  };

  const pickTemporaryAttachments = async () => {
    setChatPreview(null);
    const selected = await callFeature<Attachment[]>(
      "pick_temp_attachments",
      undefined,
      "Temporary attachment selection needs the native Kairos app.",
    );
    if (!selected?.length) return;
    setAttachments((current) => [...current, ...selected].slice(0, 5));
    setAttachmentNotice("Attachments are extracted locally, sent only with this turn, then removed from Kairos.");
  };

  const discardAttachment = (attachmentId: string) => {
    setChatPreview(null);
    setAttachments((current) => current.filter((attachment) => attachment.id !== attachmentId));
    if (nativeRuntime) void invoke("discard_temp_attachment", { attachmentId }).catch(() => undefined);
  };

  const handlePullModel = async (model: string) => {
    setModelOperation({ kind: "pull", model });
    setPullProgress({ model, status: "Starting download…" });
    setFailedPullModel(null);
    setSetupFeedback(null);
    try {
      const result = await callFeature<LocalSetupStatus>(
        "pull_ollama_model",
        { model },
        "In-app model download is not available in this native build yet. You can install it with Ollama, then refresh Kairos.",
      );
      if (result) {
        setLocalSetup(result);
        setSetupFeedback(`${model} is ready to select.`);
        await refreshStatus();
      } else if (nativeRuntime) {
        setFailedPullModel(model);
        setSetupFeedback(`${model} did not finish downloading. Check Ollama, then retry.`);
      }
    } finally {
      setPullProgress(null);
      setModelOperation(null);
    }
  };

  const cancelPullModel = async (model: string) => {
    await callFeature<unknown>(
      "cancel_ollama_pull",
      { model },
      "Kairos could not cancel this download. Ollama may still be working in the background.",
    );
    setPullProgress(null);
    setModelOperation(null);
    setSetupFeedback(`${model} download cancelled. You can retry whenever you are ready.`);
  };

  const handleTestModel = async (model: string) => {
    setModelOperation({ kind: "test", model });
    setSetupFeedback(null);
    const result = await callFeature<unknown>(
      "test_ollama_model",
      { model, contextWindowTokens: 32_768 },
      "The model test is not available in this native build yet.",
    );
    const raw = asRecord(result);
    if (result) {
      setSetupFeedback(typeof raw?.message === "string" ? raw.message : `${model} passed a real local 32K-context test.`);
      await refreshStatus();
    }
    setModelOperation(null);
  };

  const openOllamaInstall = async () => {
    if (!nativeRuntime) {
      window.open(ollamaInstallUrl, "_blank", "noopener,noreferrer");
      return;
    }
    try {
      await invoke("open_ollama_install_page");
    } catch {
      window.open(ollamaInstallUrl, "_blank", "noopener,noreferrer");
      setCapabilityNotice("Opened the official Ollama download page in your browser.");
    }
  };

  const applyHardwareProfile = async (nextHardwareProfile: HardwareProfile) => {
    setHardwareProfile(nextHardwareProfile);
    setHardwareProfileBusy(true);
    setSetupFeedback(null);
    const result = await callFeature<LocalSetupStatus>(
      "set_hardware_profile",
      { hardwareProfile: nextHardwareProfile },
      "Kairos could not save this hardware planning profile. Your selected model and context were not changed.",
    );
    if (result) {
      setLocalSetup(result);
      setShowAllModels(false);
      setSetupFeedback("Hardware planning profile saved. Your selected model, memory budget, and 32K target remain unchanged.");
      await refreshStatus();
    } else if (nativeRuntime) {
      setHardwareProfile(localSetup?.selectedHardwareProfile ?? "auto");
    }
    setHardwareProfileBusy(false);
  };

  const applyMemoryBudget = async (nextBudget: MemoryBudget) => {
    const memoryBudgetMode = nextBudget === "auto"
      ? "auto"
      : nextBudget === "custom"
        ? "custom"
        : "preset";
    const memoryBudgetGb = nextBudget === "auto"
      ? null
      : nextBudget === "custom"
        ? customMemoryBudgetValue
        : nextBudget;
    setMemoryBudget(nextBudget);
    setMemoryBudgetBusy(true);
    setSetupFeedback(null);
    const result = await callFeature<LocalSetupStatus>(
      "set_memory_budget",
      { memoryBudgetMode, memoryBudgetGb },
      "Kairos could not save this memory plan. Your selected model and context were not changed.",
    );
    if (result) {
      setLocalSetup(result);
      setShowAllModels(false);
      setSetupFeedback(`Memory plan saved: ${memoryBudgetMode === "auto" ? "Auto" : `${memoryBudgetGb} GB`}. Your model and 32K target remain unchanged.`);
      await refreshStatus();
    }
    setMemoryBudgetBusy(false);
  };

  const pickBrainFolder = async () => {
    const picked = await callFeature<unknown>(
      "pick_brain_folder",
      undefined,
      "Folder selection needs the native Kairos app. No filesystem access is exposed to this view.",
    );
    const raw = asRecord(picked);
    if (!raw || typeof raw.selectionToken !== "string") return;
    const inspection = await callFeature<unknown>(
      "inspect_brain_folder",
      { selectionToken: raw.selectionToken },
      "Folder inspection is not available in this native build yet.",
    );
    const details = asRecord(inspection);
    setAddBrain({
      selectionToken: raw.selectionToken,
      displayPath: typeof raw.displayPath === "string" ? raw.displayPath : undefined,
      name: typeof details?.suggestedName === "string" ? details.suggestedName : "New Obsidian brain",
      role: typeof details?.suggestedRole === "string" ? details.suggestedRole : "Personal knowledge vault",
      routingHints: Array.isArray(details?.routingHints) ? details?.routingHints.filter((hint): hint is string => typeof hint === "string").join(", ") : "",
      egressPolicy: "local_only",
      writePolicy: "confirm_every_write",
      graphEnabled: true,
      noteCount: typeof details?.noteCount === "number" ? details.noteCount : undefined,
      routerCandidates: Array.isArray(details?.routerCandidates)
        ? details.routerCandidates.filter((path): path is string => typeof path === "string")
        : [],
      contextCandidates: Array.isArray(details?.contextCandidates)
        ? details.contextCandidates.filter((path): path is string => typeof path === "string")
        : [],
    });
  };

  const createBrain = async () => {
    if (!addBrain) return;
    const result = await callFeature<unknown>(
      "create_brain",
      {
        request: {
          selectionToken: addBrain.selectionToken,
          name: addBrain.name,
          role: addBrain.role,
          routingHints: addBrain.routingHints.split(",").map((hint) => hint.trim()).filter(Boolean),
          egressPolicy: addBrain.egressPolicy,
          writePolicy: addBrain.writePolicy,
          graphEnabled: addBrain.graphEnabled,
        },
      },
      "Creating a brain needs the native Kairos registration update. Nothing has been written.",
    );
    if (result) {
      setAddBrain(null);
      await refreshBrains();
      await refreshGraph();
      const created = normalizeBrains([result])?.[0];
      if (created) {
        setWriteDraft((current) => current.brainId ? current : { ...current, brainId: created.id });
      }
    }
  };

  const updateBrainPolicy = async () => {
    if (!policyEditor) return;
    setPolicyBusy(true);
    setPolicyFeedback(null);
    const result = await callFeature<unknown>(
      "update_brain_policy",
      {
        brainId: policyEditor.brainId,
        egressPolicy: policyEditor.egressPolicy,
        writePolicy: policyEditor.writePolicy,
        graphEnabled: policyEditor.graphEnabled,
      },
      "Kairos could not update this brain policy. Nothing changed.",
    );
    const updatedBrain = normalizeBrains([result])?.[0];
    if (updatedBrain) {
      const nextBrains = brains.map((brain) => brain.id === updatedBrain.id ? { ...brain, ...updatedBrain } : brain);
      const nextWritableBrains = nextBrains.filter(canConfirmWrites);
      setBrains(nextBrains);
      setWriteDraft((current) => canConfirmWrites(nextBrains.find((brain) => brain.id === current.brainId) ?? {
        id: "",
        name: "",
        role: "",
      })
        ? current
        : { ...current, brainId: nextWritableBrains[0]?.id ?? "" });
      setPolicyEditor(null);
      setPolicyFeedback(`${updatedBrain.name} policy saved.`);
    }
    setPolicyBusy(false);
  };

  const savePreferences = async () => {
    setPreferencesFeedback(null);
    const result = await callFeature<unknown>(
      "save_app_preferences",
      {
        shortcut: preferences.shortcut,
        summonTarget: "compact",
        launchAtLogin: preferences.launchAtLogin,
        closeToHide: preferences.closeToHide,
        keepAboveOtherWindows: preferences.keepAboveOtherWindows,
        contextWindowTokens: preferences.contextWindowTokens,
        memoryBudgetGb: memoryBudget === "auto" ? null : effectiveMemoryBudget,
        memoryBudgetMode: memoryBudget === "auto" ? "auto" : memoryBudget === "custom" ? "custom" : "preset",
      },
      "These preferences will save once the native settings bridge is included. The preview is unchanged.",
    );
    if (result) setPreferencesFeedback("Preferences saved locally. Shortcut changes apply immediately in the desktop app.");
  };

  const saveProviderSettings = async () => {
    if (activeProvider.id === "ollama") return;
    setProviderSettingsBusy(true);
    setProviderSettingsFeedback(null);
    const result = await callFeature<AppStatus>(
      "save_provider_settings",
      {
        providerId,
        model: providerModel.trim() || undefined,
        apiKey: providerApiKey.trim() || undefined,
        enabled: true,
        makeActive: true,
      },
      "Provider configuration needs the native Kairos update. No key was stored.",
    );
    if (result) {
      setStatus(result);
      setProviderApiKey("");
      setProviderSettingsFeedback(`${activeProvider.label} is configured. Kairos will still show every outbound turn before sending it.`);
    }
    setProviderSettingsBusy(false);
  };

  const previewNoteWrite = async () => {
    if (!brains.some(canConfirmWrites)) {
      setWriteFeedback("No connected brain currently permits note writes. Set a brain to Confirm every write before preparing a diff.");
      return;
    }
    setWriteBusy(true);
    setWriteFeedback(null);
    const proposal = await callFeature<NoteWriteProposal>(
      "draft_note_write",
      writeDraft,
      "Kairos could not prepare a safe write preview. Nothing was changed.",
    );
    if (proposal) setWriteProposal(proposal);
    setWriteBusy(false);
  };

  const confirmNoteWrite = async () => {
    if (!writeProposal) return;
    setWriteBusy(true);
    setWriteFeedback(null);
    const result = await callFeature<{ relativePath?: string }>(
      "confirm_note_write",
      { proposalId: writeProposal.id, nonce: writeProposal.nonce },
      "Kairos could not confirm this write. The note was not changed.",
    );
    if (result) {
      setWriteFeedback(`Saved ${result.relativePath ?? writeProposal.relativePath}.`);
      setWriteProposal(null);
      setWriteDraft((current) => ({ ...current, markdown: "" }));
      await refreshGraph();
    }
    setWriteBusy(false);
  };

  const graphNodes = useMemo(() => graph.nodes.filter((node) => {
    const matchesBrain = graphFilter === "all" || node.brainId === graphFilter || node.id === "kairos";
    const needle = graphSearch.trim().toLowerCase();
    const matchesSearch = isKairosGraphNode(node) || !needle || node.label.toLowerCase().includes(needle);
    return matchesBrain && matchesSearch && !node.protected;
  }), [graph, graphFilter, graphSearch]);
  const graphNodeIds = new Set(graphNodes.map((node) => node.id));
  const graphEdges = graph.edges.filter((edge) => graphNodeIds.has(edge.source) && graphNodeIds.has(edge.target));
  const graphNodeById = useMemo(() => new Map(graphNodes.map((node) => [node.id, node])), [graphNodes]);
  const graphWorldNodes = useMemo(() => graphNodes.filter((node) => !isKairosGraphNode(node)), [graphNodes]);
  const graphWorldEdges = useMemo(() => graphEdges.filter((edge) => edge.source !== "kairos" && edge.target !== "kairos"), [graphEdges]);
  const graphAnchorEdges = useMemo(() => graphEdges.flatMap((edge) => {
    if (edge.source !== "kairos" && edge.target !== "kairos") return [];
    const node = graphNodeById.get(edge.source === "kairos" ? edge.target : edge.source);
    return node && !isKairosGraphNode(node) ? [{ edge, node }] : [];
  }), [graphEdges, graphNodeById]);
  const latestRoutes = useMemo(() => [...chatMessages]
    .reverse()
    .find((message) => message.role === "assistant" && message.routes?.length)?.routes ?? [], [chatMessages]);
  const latestRouteIds = useMemo(() => new Set(latestRoutes.map((route) => route.id)), [latestRoutes]);
  const graphLegend = useMemo(() => Array.from(new Set(graph.nodes
    .map((node) => node.brainId)
    .filter((brainId) => brainId && brainId !== "kairos")))
    .sort((left, right) => left.localeCompare(right))
    .map((brainId) => ({ id: brainId, ...brainVisual(brainId) })), [graph]);

  const clampGraphScale = (scale: number) => Math.max(graphZoomMin, Math.min(graphZoomMax, scale));
  const zoomGraphAt = (nextScale: number, focusX: number, focusY: number) => {
    setGraphCamera((current) => {
      const scale = clampGraphScale(nextScale);
      const worldX = (focusX - current.x) / current.scale;
      const worldY = (focusY - current.y) / current.scale;
      return {
        scale,
        x: focusX - worldX * scale,
        y: focusY - worldY * scale,
      };
    });
  };
  const zoomGraphBy = (amount: number) => {
    zoomGraphAt(
      graphCamera.scale + amount,
      graphViewportSize.width / 2,
      graphViewportSize.height / 2,
    );
  };
  const fitGraph = () => {
    setPreviousGraphCamera(null);
    setSelectedGraphNode(null);
    setGraphCamera(graphCameraFit);
  };
  const focusGraphNode = (node: GraphNode) => {
    if (isKairosGraphNode(node)) {
      fitGraph();
      return;
    }
    const scale = Math.max(1.8, graphCamera.scale);
    setPreviousGraphCamera(graphCamera);
    setSelectedGraphNode(node.id);
    setGraphCamera({
      scale,
      x: graphViewportSize.width * 0.5 - ((node.x ?? 50) / 100) * graphViewportSize.width * scale,
      y: graphViewportSize.height * 0.5 - ((node.y ?? 50) / 100) * graphViewportSize.height * scale,
    });
  };
  const restoreGraphCamera = () => {
    setSelectedGraphNode(null);
    if (previousGraphCamera) setGraphCamera(previousGraphCamera);
    setPreviousGraphCamera(null);
  };
  const handleGraphWheel = (event: ReactWheelEvent<HTMLDivElement>) => {
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    const delta = event.deltaY > 0 ? -0.16 : 0.16;
    zoomGraphAt(graphCamera.scale + delta, event.clientX - bounds.left, event.clientY - bounds.top);
  };
  const handleGraphPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest("button, input, .graph-inspector, .graph-minimap")) return;
    graphDragRef.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      camera: graphCamera,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const handleGraphPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const drag = graphDragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setGraphCamera({
      ...drag.camera,
      x: drag.camera.x + event.clientX - drag.startX,
      y: drag.camera.y + event.clientY - drag.startY,
    });
  };
  const finishGraphPointer = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (graphDragRef.current?.pointerId !== event.pointerId) return;
    graphDragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  };
  const centreGraphFromMinimap = (clientX: number, clientY: number, element: SVGSVGElement) => {
    const bounds = element.getBoundingClientRect();
    const x = Math.max(0, Math.min(1, (clientX - bounds.left) / bounds.width));
    const y = Math.max(0, Math.min(1, (clientY - bounds.top) / bounds.height));
    setGraphCamera((current) => ({
      ...current,
      x: graphViewportSize.width / 2 - x * graphViewportSize.width * current.scale,
      y: graphViewportSize.height / 2 - y * graphViewportSize.height * current.scale,
    }));
  };
  const minimapViewportWidth = Math.min(100, 100 / graphCamera.scale);
  const minimapViewportHeight = Math.min(100, 100 / graphCamera.scale);
  const minimapViewport = {
    x: Math.max(0, Math.min(100 - minimapViewportWidth, (-graphCamera.x / graphCamera.scale / graphViewportSize.width) * 100)),
    y: Math.max(0, Math.min(100 - minimapViewportHeight, (-graphCamera.y / graphCamera.scale / graphViewportSize.height) * 100)),
    width: minimapViewportWidth,
    height: minimapViewportHeight,
  };

  useEffect(() => {
    if (selectedGraphNode && !graphNodeIds.has(selectedGraphNode)) {
      setSelectedGraphNode(null);
      setPreviousGraphCamera(null);
      setGraphCamera(graphCameraFit);
    }
  }, [graphNodes, selectedGraphNode]);

  const showView = (nextView: ViewId) => {
    if (nextView === "settings") setSettingsAnchor("top");
    setView(nextView);
    setSurfaceMode("cockpit");
    setCapabilityNotice(null);
  };

  const setupState = !status?.initialized
    ? { title: "Connect your brains", detail: "Create the local Kairos registry before model and vault settings can be saved.", tone: "warning" as const }
    : !ollamaRunning
      ? { title: localSetup?.ollamaInstalled === false ? "Install Ollama" : "Start Ollama", detail: localSetup?.setupMessage ?? "Kairos cannot reach Ollama at localhost:11434.", tone: "danger" as const }
      : !selectedModelInstalled
        ? { title: "Download the selected model", detail: status?.model.setupMessage ?? "The selected model is not installed yet.", tone: "warning" as const }
        : { title: "Local AI is ready", detail: `${activeModel} · ${status?.model.contextWindowTokens ? status.model.contextWindowTokens / 1024 : 32}K context · local only`, tone: "success" as const };
  const planningBusy = memoryBudgetBusy || hardwareProfileBusy;
  const currentModelFit = selectedModelCard?.fit ?? selectedModelFit;
  const scrollToRecommendations = () => {
    document.getElementById("recommended-models")?.scrollIntoView({ behavior: "smooth", block: "start" });
  };
  const renderModelCard = (setupModel: LocalSetupModel, kind: "recommended" | "advanced") => {
    const id = setupModel.id;
    const fallback = modelCatalog[id];
    const installed = setupModel.installed ?? isInstalledModel(id, status?.model.installedModels ?? []);
    const selected = sameModelId(status?.model.selectedModel ?? "", id);
    const working = modelOperation !== null && sameModelId(modelOperation.model, id);
    const pulling = modelOperation?.kind === "pull" && sameModelId(modelOperation.model, id);
    const progress = pulling && pullProgress && sameModelId(pullProgress.model, id) ? pullProgress : null;
    const fit = setupModel.fit;
    const knownWarning = modelKnownWarnings[normalizedModelId(id)];
    const displayName = setupModel.label ?? fallback?.label;
    const capabilities = setupModel.capabilities ?? [];
    return (
      <article className={`model-card model-card--${kind} ${selected ? "is-selected" : ""} model-card--${fit?.fit ?? "unknown"}`} key={id}>
        <div>
          <p className="section-label">{setupModel.role ?? (kind === "recommended" ? "Recommended local model" : "Advanced catalog")}</p>
          <h3><code className="model-card__tag">{id}</code></h3>
          {displayName && displayName !== id && <p className="model-card__name">{displayName}{setupModel.variant ? ` · ${setupModel.variant}` : ""}</p>}
          <p>{fit?.message ?? "Fit estimate becomes available after the local setup check."}</p>
        </div>
        <dl>
          <div><dt>Download</dt><dd>{setupModel.downloadSize ?? fallback?.downloadSize ?? "Reported by Ollama during download"}</dd></div>
          <div><dt>Context</dt><dd>32K target</dd></div>
        </dl>
        {capabilities.length > 0 && <div className="model-capability-list" aria-label="Model capabilities">{capabilities.map((capability) => <span key={capability}>{capability}</span>)}</div>}
        {setupModel.whyRecommended && <p className="model-card__why">{setupModel.whyRecommended}</p>}
        {knownWarning && <small className="model-card__warning">{knownWarning}</small>}
        <div className="model-card__actions">
          {fit && <StatusPill tone={modelFitTone(fit)}>{modelFitLabel(fit).toUpperCase()}</StatusPill>}
          {installed ? <StatusPill tone="success">INSTALLED</StatusPill> : pulling ? <div className="model-pull-progress"><progress max="100" value={progress?.percent} /><span>{progress?.percent !== undefined ? `${Math.round(progress.percent)}% · ${progress.status}` : progress?.status ?? "Starting download…"}</span><button className="secondary-action" type="button" onClick={() => void cancelPullModel(id)}>Cancel</button></div> : <button className="secondary-action" type="button" onClick={() => void handlePullModel(id)} disabled={Boolean(modelOperation) || planningBusy} title="Downloads this model only; it will not change your active model">{failedPullModel && sameModelId(failedPullModel, id) ? "Retry download" : "Download"}</button>}
          {installed && <button className="text-action" type="button" onClick={() => void handleTestModel(id)} disabled={Boolean(modelOperation) || planningBusy}>{working && modelOperation?.kind === "test" ? "Testing…" : "Test at 32K"}</button>}
          {installed && !selected && <button className="text-action" type="button" onClick={() => void selectModel(id)} disabled={busy || planningBusy}>Use this model</button>}
          {setupModel.verifiedAt32k && <StatusPill tone="success">VERIFIED 32K</StatusPill>}
        </div>
      </article>
    );
  };

  return (
    <main className={`app-shell ${expanded ? "is-expanded" : "is-compact"} view-${view}`}>
      <section className="control-plane">
        <header className="app-header">
          <div className="brand-lockup">
            <img className="brand-mark" src={brandMark} alt="Kairos" />
            <img className="brand-wordmark" src={brandWordmark} alt="Kairos — Knowledge, Action, Intelligence, Routing" />
          </div>
          <div className="header-actions">
            <div className="summon-key" title="Toggle Kairos">
              <span>SUMMON</span>
              <kbd>⌥ Space</kbd>
            </div>
            <button className="icon-button expand-button" onClick={() => {
              if (expanded) {
                setView("chat");
                setSurfaceMode("compact");
              } else {
                setView("chat");
                setSurfaceMode("cockpit");
              }
            }} aria-label={expanded ? "Use compact chat" : "Expand Kairos"}>
              {expanded ? "⌃" : "⌄"}
            </button>
          </div>
        </header>

        {expanded && (
          <nav className="primary-nav" aria-label="Kairos sections">
            {([
              ["chat", "Cockpit", "⌁"],
              ["history", "Chat history", "◌"],
              ["next", "What Next", "✦"],
              ["map", "Brain Map", "⌘"],
              ["brains", "Connected brains", "▱"],
              ["privacy", "Privacy and providers", "◇"],
              ["settings", "Settings", "⚙"],
            ] as Array<[NavId, string, string]>).map(([id, label, icon]) => (
              <button
                key={id}
                className={`nav-button ${(
                  id === "history"
                    ? historyOpen
                    : id === "brains" || id === "privacy"
                      ? view === "settings" && settingsAnchor === id
                      : id === "settings"
                        ? view === "settings" && settingsAnchor === "top"
                        : view === id
                ) ? "is-active" : ""}`}
                onClick={() => {
                  if (id === "history") {
                    setView("chat");
                    setHistoryOpen((open) => !open);
                  } else if (id === "brains" || id === "privacy") {
                    setHistoryOpen(false);
                    setSettingsAnchor(id);
                    setView("settings");
                    setSurfaceMode("cockpit");
                  } else {
                    setHistoryOpen(false);
                    showView(id);
                  }
                }}
                aria-current={id !== "history" && view === id ? "page" : undefined}
                aria-label={label}
                title={label}
              >
                <span className="nav-button__icon" aria-hidden="true">{icon}</span>
                <span className="nav-button__label">{label}</span>
              </button>
            ))}
          </nav>
        )}

        {!nativeRuntime && (
          <p className="preview-notice">
            Browser preview · local routing, downloads, and vault access activate in the Kairos desktop app.
          </p>
        )}

        {capabilityNotice && (
          <aside className="capability-notice" role="status">
            <span>{capabilityNotice}</span>
            <button onClick={() => setCapabilityNotice(null)} aria-label="Dismiss notice">×</button>
          </aside>
        )}

        {error && <p className="error-state" role="alert">{error}</p>}

        {(view === "chat" || expanded) && (
          <section className="chat-surface" aria-label="Kairos chat">
            <div className="chat-heading">
              <div>
                <p className="section-label">Kairos chat</p>
                <h1>{expanded ? "Ask from the right context." : "What matters now?"}</h1>
              </div>
              <StatusPill tone={activeProvider.external ? "warning" : "local"}>
                {activeProvider.external ? "PREVIEW FIRST" : "LOCAL"}
              </StatusPill>
            </div>

            {!status?.initialized && (
              <div className="setup-inline setup-inline--onboarding">
                <div><strong>Connect your first brain.</strong><span>Choose an existing folder yourself. Kairos indexes in place, starts local-only, and does not copy your notes.</span></div>
                <button className="primary-action" type="button" onClick={() => showView("settings")}>Set up Kairos</button>
              </div>
            )}

            <div className="chat-session-bar">
              <button className="secondary-action" type="button" onClick={() => void startNewChat()} disabled={busy}>New chat</button>
              {chatMessages.some((message) => !message.notice) && <button className="text-action" type="button" onClick={prepareChatLog} disabled={busy}>Save chat to brain</button>}
              {expanded && historyOpen && <label className="graph-search"><span className="sr-only">Search chat history</span><input value={chatSearch} onChange={(event) => { setChatSearch(event.target.value); void refreshChatSessions(event.target.value); }} placeholder="Search local chats" /></label>}
              {expanded && chatSessionId && <button className="text-action" type="button" onClick={() => void deleteChatSession(chatSessionId)} disabled={busy}>Delete chat</button>}
            </div>

            {expanded && historyOpen && chatSessions.length > 0 && (
              <aside className="chat-history" aria-label="Local chat history">
                {chatSessions.map((session) => <div key={session.id} className={`chat-history__item ${session.id === chatSessionId ? "is-active" : ""}`}><button type="button" onClick={() => void loadChatSession(session.id)}><strong>{session.title}</strong><small>{session.messageCount} messages · {shortDate(session.updatedAt)}</small></button>{session.id !== chatSessionId && <button type="button" className="text-action" onClick={() => void deleteChatSession(session.id)} aria-label={`Delete ${session.title}`}>×</button>}</div>)}
              </aside>
            )}

            <div className="chat-context-bar chat-context-bar--route-only">
              <label>
                <span>Route</span>
                <select
                  value={brainOverride}
                  onChange={(event) => {
                    setBrainOverride(event.target.value);
                    if (chatPreview) setChatPreview(null);
                  }}
                  disabled={busy || Boolean(chatPreview)}
                >
                  <option value="auto">Auto-route</option>
                  {brains.filter((brain) => brain.enabled !== false).map((brain) => (
                    <option key={brain.id} value={brain.id}>{brain.name}</option>
                  ))}
                </select>
              </label>
            </div>

            <div className="chat-thread" aria-live="polite">
              {chatMessages.map((message) => (
                <article key={message.id} className={`chat-message chat-message--${message.role} ${message.notice ? "is-notice" : ""}`}>
                  <div className="message-meta">
                    <span>{message.role === "assistant" ? "Kairos" : "You"}</span>
                    {message.provider && <span>{message.provider}</span>}
                    <time>{shortDate(message.createdAt)}</time>
                  </div>
                  <p>{message.body}</p>
                  <RouteChips routes={message.routes} />
                  {message.citations?.length ? (
                    <div className="citation-row">
                      {message.citations.map((citation) => <CitationChip key={citation.id} citation={citation} />)}
                    </div>
                  ) : null}
                </article>
              ))}
              {streamingAssistant && (
                <article className="chat-message chat-message--assistant is-streaming" aria-label="Kairos is replying">
                  <div className="message-meta"><span>Kairos</span><span>Ollama · local</span><time>{shortDate(streamingAssistant.createdAt)}</time></div>
                  {streamingAssistant.body ? <p>{streamingAssistant.body}<span className="stream-caret" aria-hidden="true" /></p> : <div className="stream-pending"><KairosLoader /><span>Composing from approved local context…</span></div>}
                </article>
              )}
              {busy && !streamingAssistant && (
                <article className="chat-message chat-message--assistant is-loading">
                  <KairosLoader />
                  <span>{activeProvider.external ? "Preparing your reviewed handoff…" : "Routing approved local context…"}</span>
                </article>
              )}
            </div>

            {chatPreview && (
              <aside className="consent-preview" aria-label="External provider preview">
                <div className="consent-preview__heading">
                  <div>
                    <p className="section-label">External handoff preview</p>
                    <h2>Review before sending</h2>
                  </div>
                  <StatusPill tone="warning">NOT SENT</StatusPill>
                </div>
                <dl className="preview-list">
                  <div><dt>Destination</dt><dd>{chatPreview.providerLabel}</dd></div>
                  <div><dt>Message</dt><dd>{chatPreview.outgoingSummary}</dd></div>
                  <div><dt>Routed brains</dt><dd>{chatPreview.routes.length ? chatPreview.routes.map((route) => route.name).join(", ") : "No route selected yet"}</dd></div>
                  <div><dt>Attachments</dt><dd>{chatPreview.attachmentNames.length ? `${chatPreview.attachmentNames.join(", ")} (temporary local extraction included)` : "None"}</dd></div>
                </dl>
                {chatPreview.citations.length > 0 && (
                  <div className="citation-row">
                    {chatPreview.citations.map((citation) => <CitationChip key={citation.id} citation={citation} />)}
                  </div>
                )}
                <div className="consent-actions">
                  <button className="secondary-action" onClick={() => setChatPreview(null)}>Cancel</button>
                  <button className="primary-action" onClick={() => void submitChat(true)} disabled={busy || !chatPreview.previewToken}>Confirm and send</button>
                </div>
              </aside>
            )}

            <form
              className="chat-composer"
              onSubmit={(event) => {
                event.preventDefault();
                if (!chatCanSend) return;
                void submitChat();
              }}
            >
              {attachments.length > 0 && (
                <div className="attachment-row" aria-label="Temporary attachments">
                  {attachments.map((attachment) => (
                    <span key={attachment.id} className="attachment-chip">
                      <span>⌁</span>{attachment.name}
                      <button type="button" onClick={() => discardAttachment(attachment.id)} aria-label={`Remove ${attachment.name}`}>×</button>
                    </span>
                  ))}
                </div>
              )}
              <textarea
                ref={composerRef}
                value={composer}
                onChange={(event) => {
                  setComposer(event.target.value);
                  if (chatPreview) setChatPreview(null);
                }}
                onKeyDown={(event) => {
                  if (
                    event.key !== "Enter"
                    || event.shiftKey
                    || event.nativeEvent.isComposing
                    || event.nativeEvent.keyCode === 229
                  ) return;
                  event.preventDefault();
                  if (chatCanSend) void submitChat();
                }}
                placeholder="Ask Kairos about a project, decision, or next action…"
                rows={expanded ? 3 : 2}
                disabled={busy}
                aria-keyshortcuts="Enter"
              />
              <div className="composer-actions">
                <button
                  type="button"
                  className="attachment-button"
                  onClick={() => void pickTemporaryAttachments()}
                  title="Attach up to five temporary files for this turn"
                  aria-label="Attach temporary files"
                  disabled={busy}
                >
                  <span aria-hidden="true">＋</span>
                </button>
                <label className="composer-select composer-select--consent" title={activeKairosConsentMode.detail}>
                  <span className="composer-select__icon" aria-hidden="true">✋</span>
                  <span className="sr-only">Kairos execution and consent mode</span>
                  <select value={kairosConsentMode} onChange={(event) => setKairosConsentMode(event.target.value as KairosConsentMode)} aria-label="Kairos execution and consent mode" disabled={busy}>
                    {kairosConsentModes.map((mode) => <option key={mode.id} value={mode.id}>{mode.label}</option>)}
                  </select>
                </label>
                <label className="composer-select">
                  <span className="sr-only">Provider</span>
                  <select value={providerId} onChange={(event) => setProviderId(event.target.value as ProviderId)} aria-label="Provider" disabled={busy || Boolean(chatPreview)}>
                    {providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.label}</option>)}
                  </select>
                </label>
                {providerId === "ollama" && (
                  <label className="composer-select composer-select--model">
                    <span className="sr-only">Ollama model</span>
                    <select
                      value={selectedInstalledOllamaModel}
                      onChange={(event) => void selectModel(event.target.value)}
                      disabled={!status || busy || Boolean(chatPreview) || !nativeRuntime || installedOllamaChoices.length === 0}
                      aria-label="Ollama model"
                    >
                      {!status && <option value="">Checking local models…</option>}
                      {status && installedOllamaChoices.length === 0 && <option value="">No installed local model</option>}
                      {status && !selectedInstalledOllamaModel && installedOllamaChoices.length > 0 && <option value="" disabled>Current model is not installed</option>}
                      {installedOllamaChoices.map((choice) => <option key={choice.id} value={choice.id}>{choice.label}</option>)}
                    </select>
                  </label>
                )}
                <span className="composer-hint" title={composerProviderDetail}>{composerProviderDetail}</span>
                <button
                  type="submit"
                  className="composer-send-button"
                  disabled={!chatCanSend}
                  aria-label={activeProvider.external ? "Preview external message" : "Send local message"}
                  title={activeProvider.external ? "Review before sending" : "Send local message"}
                >
                  <span aria-hidden="true">↑</span>
                  <span className="sr-only">{activeProvider.external ? "Preview" : "Send"}</span>
                </button>
              </div>
              <p className="composer-consent-note">
                {activeKairosConsentMode.detail} Cloud, API, and CLI turns always show a review before sending; note writes always require a diff confirmation.
              </p>
              {attachmentNotice && <p className="attachment-notice">{attachmentNotice}</p>}
            </form>
          </section>
        )}

        {expanded && view === "next" && (
          <section className="dashboard-surface">
            <div className="surface-heading">
              <div>
                <p className="section-label">Cross-brain pulse</p>
                <h1>What should I do next?</h1>
                <p>One useful action, grounded in approved local context.</p>
              </div>
              <div className="surface-heading__actions">
                <StatusPill tone={activeProvider.external ? "warning" : "local"}>{activeProvider.external ? "PREVIEW FIRST" : "LOCAL ONLY"}</StatusPill>
                <button className="primary-action" onClick={() => void runPulse()} disabled={busy || (!activeProvider.external && !modelReady)}>
                  {busy ? "Composing…" : activeProvider.external ? "Review cloud pulse" : "Compose pulse"}
                </button>
                <button className="icon-button" type="button" onClick={() => setView("chat")} aria-label="Back to cockpit">×</button>
              </div>
            </div>

            <div className="pulse-overview">
              <div className="pulse-route-visual">{busy ? <KairosLoader /> : <img src={routeIcon} alt="" />}</div>
              <div>
                <p className="section-label">{routeLabel}</p>
                <h2>{briefResult ? briefResult.context.route.brains.map((brain) => brain.name).join(" + ") : "Your connected brains"}</h2>
                <p>{briefResult ? "Only selected evidence was included." : "Kairos will route to the smallest useful set of enabled brains."}</p>
              </div>
              <div className="pulse-status">
                <span className={`status-dot ${modelReady ? "is-ready" : "needs-setup"}`} />
                <span>{modelReady ? `${activeModel} · 32K local context` : setupState.title}</span>
              </div>
            </div>

            {!status?.initialized && (
              <div className="setup-inline">
                <div><strong>First, connect your existing brains.</strong><span>Choose each folder yourself; Kairos keeps notes in place and begins local-only.</span></div>
                <button className="secondary-action" onClick={() => showView("settings")} disabled={busy || !nativeRuntime}>Set up Kairos</button>
              </div>
            )}

            {briefResult && (
              <section className="brief-result" aria-live="polite">
                <div className="result-header">
                  <div className="route-summary">
                    <img src={routeIcon} alt="" />
                    <div><p className="section-label">Authoritative route</p><h2>{briefResult.context.route.brains.map((brain) => brain.name).join(" + ")}</h2></div>
                  </div>
                  <StatusPill tone="local">LOCAL ONLY</StatusPill>
                </div>
                <article className="selected-action">
                  <p className="section-label">Selected moment · {activeModel}</p>
                  <h3>{briefResult.answer.action}</h3>
                  <p>{briefResult.answer.why}</p>
                  <p className="action-caveat">{briefResult.answer.caveat}</p>
                  {briefResult.answer.sourceIds.length > 0 && (
                    <div className="citation-row">
                      {briefResult.context.sources.filter((source) => briefResult.answer.sourceIds.includes(source.source.id)).slice(0, 3).map(({ source }) => (
                        <CitationChip key={source.id} citation={sourceCitation(source)} />
                      ))}
                    </div>
                  )}
                </article>
                {briefResult.context.freshnessWarnings.length > 0 && (
                  <aside className="freshness-notice">{briefResult.context.freshnessWarnings.map((warning) => <p key={warning}>{warning}</p>)}</aside>
                )}
                <details className="evidence-drawer">
                  <summary><span>Approved local evidence</span><span>{briefResult.context.sources.length} source{briefResult.context.sources.length === 1 ? "" : "s"}</span></summary>
                  <div className="evidence-list">
                    {briefResult.context.sources.map(({ source, content, truncated }) => (
                      <article className="evidence-item" key={source.id}>
                        <div className="evidence-item__meta"><CitationChip citation={sourceCitation(source)} />{source.modifiedAt && <time>{shortDate(source.modifiedAt)}</time>}</div>
                        <pre>{content}</pre>
                        {truncated && <small>Bounded before reaching the model.</small>}
                      </article>
                    ))}
                  </div>
                </details>
                {briefResult.context.withheldSources.length > 0 && <p className="withheld-note">Protected sources were withheld by policy.</p>}
              </section>
            )}
          </section>
        )}

        {expanded && (
          <section className="map-surface cockpit-map">
            <div className="surface-heading">
              <div>
                <p className="section-label">Whole-brain atlas</p>
                <h1>See the shape of your memory.</h1>
                <p>Explicit Markdown links, tags, and registered bridges only. Protected notes never appear here.</p>
              </div>
              <button className="secondary-action" onClick={() => void refreshGraph()} disabled={graphBusy}>{graphBusy ? "Refreshing…" : "Refresh metadata"}</button>
            </div>

            <div className="graph-toolbar">
              <div className="graph-filters" aria-label="Graph filters">
                <button className={graphFilter === "all" ? "is-active" : ""} onClick={() => setGraphFilter("all")}>All brains</button>
                {brains.filter((brain) => brain.graphEnabled !== false && brain.enabled !== false).map((brain) => (
                  <button key={brain.id} className={graphFilter === brain.id ? "is-active" : ""} onClick={() => setGraphFilter(brain.id)}>{brain.name.replace(" Brain", "")}</button>
                ))}
              </div>
              <label className="graph-search"><span className="sr-only">Search graph</span><input value={graphSearch} onChange={(event) => setGraphSearch(event.target.value)} placeholder="Find a note or map" /></label>
            </div>
            <div className="graph-legend" aria-label="Brain colours and link types">
              {graphLegend.map((brain) => <span key={brain.id}><i style={{ "--legend-color": brain.color } as CSSProperties} />{brain.label}</span>)}
              <span className="graph-legend__edge"><i />Cross-brain explicit link</span>
            </div>

            <div className="graph-layout">
              <div
                className={`graph-canvas ${graphDragRef.current ? "is-panning" : ""}`}
                aria-label="Kairos whole brain graph"
                ref={graphCanvasRef}
                onWheel={handleGraphWheel}
                onPointerDown={handleGraphPointerDown}
                onPointerMove={handleGraphPointerMove}
                onPointerUp={finishGraphPointer}
                onPointerCancel={finishGraphPointer}
              >
                <svg className="graph-anchor-lines" aria-hidden="true">
                  {graphAnchorEdges.map(({ edge, node }) => {
                    const targetX = graphCamera.x + ((node.x ?? 50) / 100) * graphViewportSize.width * graphCamera.scale;
                    const targetY = graphCamera.y + ((node.y ?? 50) / 100) * graphViewportSize.height * graphCamera.scale;
                    const active = latestRouteIds.has(node.brainId);
                    return <line key={edge.id ?? `kairos-${node.id}`} className={active ? "is-active-route" : ""} x1="0" y1={graphViewportSize.height / 2} x2={targetX} y2={targetY} />;
                  })}
                </svg>
                <div
                  className="graph-world"
                  style={{ transform: `translate3d(${graphCamera.x}px, ${graphCamera.y}px, 0) scale(${graphCamera.scale})` }}
                >
                  <svg className="graph-lines" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
                  {graphWorldEdges.map((edge) => {
                    const source = graphNodeById.get(edge.source);
                    const target = graphNodeById.get(edge.target);
                    if (!source || !target) return null;
                    const kind = edge.kind?.toLowerCase() ?? "";
                    const explicitLink = ["wiki_link", "markdown_link", "embed"].includes(kind);
                    const crossBrain = explicitLink && source.brainId !== target.brainId && source.brainId !== "kairos" && target.brainId !== "kairos";
                    return <line key={edge.id ?? `${edge.source}-${edge.target}-${edge.kind ?? ""}`} className={`${kind === "contains" ? "is-containment" : ""} ${kind === "tag" ? "is-tag" : ""} ${crossBrain ? "is-cross-brain" : ""}`} x1={source.x} y1={source.y} x2={target.x} y2={target.y} />;
                  })}
                  </svg>
                {graphWorldNodes.map((node) => {
                  const visual = brainVisual(node.brainId);
                  const showLabel = selectedGraphNode === node.id || hoveredGraphNode === node.id || (Boolean(graphSearch.trim()) && graphWorldNodes.length <= 8);
                  const labelLeft = (node.x ?? 50) > 65;
                  return (
                    <button
                      key={node.id}
                      className={`graph-node graph-node--${node.kind ?? "note"} ${selectedGraphNode === node.id ? "is-selected" : ""} ${showLabel ? "shows-label" : ""} ${labelLeft ? "label-left" : ""}`}
                      style={{ "--node-color": visual.color, left: `${node.x ?? 50}%`, top: `${node.y ?? 50}%` } as CSSProperties}
                      onClick={() => focusGraphNode(node)}
                      onMouseEnter={() => setHoveredGraphNode(node.id)}
                      onMouseLeave={() => setHoveredGraphNode((current) => current === node.id ? null : current)}
                      onFocus={() => setHoveredGraphNode(node.id)}
                      onBlur={() => setHoveredGraphNode((current) => current === node.id ? null : current)}
                      title={`${node.label} · ${visual.label}`}
                    >
                      <span className="graph-node__dot" />
                      {showLabel && <span className="graph-node__label">{node.label}</span>}
                    </button>
                  );
                })}
                </div>
                <button
                  className={`kairos-seam-node ${streamingAssistant || busy ? "is-routing" : ""}`}
                  type="button"
                  onClick={fitGraph}
                  title={latestRoutes.length ? `Routed to ${latestRoutes.map((route) => route.name).join(", ")}` : "Kairos · fit whole brain"}
                  aria-label="Kairos routing centre. Fit the whole brain graph."
                >
                  <span />
                </button>
                <div className="graph-camera-controls" aria-label="Graph zoom controls">
                  <button type="button" onClick={() => zoomGraphBy(-0.2)} aria-label="Zoom out">−</button>
                  <button type="button" onClick={() => zoomGraphBy(0.2)} aria-label="Zoom in">＋</button>
                  <button type="button" onClick={fitGraph}>Fit</button>
                  <button type="button" onClick={restoreGraphCamera} disabled={!previousGraphCamera}>Back</button>
                  <span>{Math.round(graphCamera.scale * 100)}%</span>
                </div>
                <svg
                  className="graph-minimap"
                  viewBox="0 0 100 100"
                  preserveAspectRatio="none"
                  aria-label="Graph minimap"
                  onPointerDown={(event) => centreGraphFromMinimap(event.clientX, event.clientY, event.currentTarget)}
                  onPointerMove={(event) => {
                    if (event.buttons === 1) centreGraphFromMinimap(event.clientX, event.clientY, event.currentTarget);
                  }}
                >
                  {graphWorldEdges.map((edge) => {
                    const source = graphNodeById.get(edge.source);
                    const target = graphNodeById.get(edge.target);
                    return source && target ? <line key={`mini-${edge.id ?? `${edge.source}-${edge.target}`}`} x1={source.x} y1={source.y} x2={target.x} y2={target.y} /> : null;
                  })}
                  {graphWorldNodes.map((node) => <circle key={`mini-${node.id}`} cx={node.x} cy={node.y} r={node.kind === "brain" ? 1.7 : 1} fill={brainVisual(node.brainId).color} />)}
                  <rect className="graph-minimap__viewport" x={minimapViewport.x} y={minimapViewport.y} width={minimapViewport.width} height={minimapViewport.height} />
                </svg>
                {graphWorldNodes.length === 0 && <div className="graph-empty">No matching public graph metadata.</div>}
                {graphNode && (
                  <aside className="graph-inspector">
                    <div className="graph-inspector__heading">
                      <div><p className="section-label">Selected node</p><h2>{graphNode.label}</h2></div>
                      <button className="icon-button" type="button" onClick={restoreGraphCamera} aria-label="Close node details">×</button>
                    </div>
                    <RouteChips routes={[{ id: graphNode.brainId, name: brainVisual(graphNode.brainId).label }]} />
                    <p>{graphNode.kind === "bridge" ? "Registered cross-brain bridge" : graphNode.kind === "tag" ? "Tag metadata, not a file" : "Explicit metadata node"}</p>
                    {graphNode.relativePath && <code>{graphNode.relativePath}</code>}
                    {graphNode.kind === "tag" ? (
                      <div className="graph-inspector__connections"><strong>Connected notes</strong>{graphEdges.filter((edge) => edge.source === graphNode.id || edge.target === graphNode.id).map((edge) => graphNodeById.get(edge.source === graphNode.id ? edge.target : edge.source)).filter((node): node is GraphNode => Boolean(node && node.kind !== "tag" && !isKairosGraphNode(node))).map((node) => <button key={node.id} className="text-action" onClick={() => focusGraphNode(node)}>{node.label}</button>)}</div>
                    ) : graphNode.kind === "note" || graphNode.kind === "bridge" || graphNode.kind === "brain" ? (
                      <button className="secondary-action" onClick={() => void revealGraphNode(graphNode.id)}>{graphNode.kind === "brain" ? "Open brain folder" : "Reveal in Finder"}</button>
                    ) : null}
                    <small>Metadata only. Kairos rechecks the source before Finder opens it.</small>
                  </aside>
                )}
              </div>
            </div>
          </section>
        )}

        {expanded && view === "settings" && (
          <section className="settings-surface">
            <div className="surface-heading">
              <div>
                <p className="section-label">Control room</p>
                <h1>Make Kairos yours.</h1>
                <p>Brains remain authoritative. Kairos stores only connections, policies, and local app settings.</p>
              </div>
              <div className="surface-heading__actions">
                <button className="secondary-action" onClick={() => void refreshStatus()}>Refresh status</button>
                <button className="icon-button" type="button" onClick={() => setView("chat")} aria-label="Back to cockpit">×</button>
              </div>
            </div>

            <section className="settings-card local-setup-card">
              <div className="settings-card__heading">
                <div><p className="section-label">Local AI setup</p><h2>{setupState.title}</h2><p>{setupState.detail}</p></div>
                <StatusPill tone={setupState.tone}>{ollamaRunning ? "OLLAMA" : "SETUP"}</StatusPill>
              </div>
              <div className="hardware-grid">
                <div><span>{primaryFitLimitLabel}</span><strong>{primaryFitLimitGb ? `${primaryFitLimitGb} GB` : "Detecting…"}</strong><small>{hardwareProfileLabel}{hardwarePlanningOverride ? " · Manual planning override." : " · Auto-detected."}</small></div>
                <div><span>Free disk</span><strong>{availableDiskGb ? `${availableDiskGb} GB` : "Detecting…"}</strong><small>Download packages are stored by Ollama.</small></div>
                <div><span>Recommendation context</span><strong>32K</strong><small>Fit and tests stay at 32K; Kairos never lowers it silently.</small></div>
              </div>
              <div className="memory-setup-row">
                <div className="memory-controls">
                  <label className="hardware-profile-control"><span>Hardware profile</span><select value={hardwareProfile} disabled={planningBusy} onChange={(event) => void applyHardwareProfile(event.target.value as HardwareProfile)}>
                    <option value="auto">Auto ({hardwareProfileLabel})</option>
                    <optgroup label="Manual planning override">
                      <option value="apple_unified">Apple unified memory</option>
                      <option value="nvidia_vram">NVIDIA VRAM</option>
                      <option value="cpu_only">CPU-only</option>
                    </optgroup>
                  </select></label>
                  <label className="memory-budget-control"><span>Memory budget</span><select value={String(memoryBudget)} disabled={planningBusy} onChange={(event) => {
                    const nextBudget = (event.target.value === "auto" || event.target.value === "custom" ? event.target.value : Number(event.target.value)) as MemoryBudget;
                    setMemoryBudget(nextBudget);
                    if (nextBudget !== "custom") void applyMemoryBudget(nextBudget);
                  }}>
                    <option value="auto">{primaryFitLimitGb ? `Auto (${primaryFitLimitGb} GB fit limit)` : "Auto (detect hardware)"}</option>
                    {[16, 24, 32, 48, 64, 96, 192].map((amount) => <option key={amount} value={amount}>{amount} GB</option>)}
                    <option value="custom">Custom</option>
                  </select></label>
                  {memoryBudget === "custom" && <label className="custom-memory-budget-control"><span>Custom budget (GB)</span><input type="number" min="1" max="192" value={customMemoryBudget} disabled={planningBusy} onChange={(event) => setCustomMemoryBudget(event.target.value)} /></label>}
                  <div className="memory-budget-copy">
                    <strong>{effectiveMemoryBudget} GB preference cap</strong>
                    <p>{localSetup?.memoryBudgetMessage ?? "This is a planning preference, not a claim about installed hardware. Changing it re-ranks guidance only; Kairos does not change your model or context automatically."}</p>
                    <p>Fit uses {primaryFitLimitLabel.toLowerCase()}{primaryFitLimitGb ? ` (${primaryFitLimitGb} GB)` : ""} for one 32K conversation.</p>
                    {memoryBudgetPending && <span className="memory-plan-pending">Pending apply — the shortlist below still reflects your saved budget.</span>}
                    {memoryBudget === "custom" && <button className="text-action" type="button" onClick={() => void applyMemoryBudget("custom")} disabled={planningBusy}>Apply custom budget</button>}
                  </div>
                </div>
                <aside className={`ollama-health ${ollamaRunning ? "is-running" : "is-warning"}`} aria-label="Ollama setup health">
                  <div className="ollama-health__heading"><div><p className="section-label">Ollama</p><h3>{!ollamaInstalled ? "Not installed" : ollamaRunning ? "Running locally" : "Installed, not running"}</h3></div><StatusPill tone={ollamaRunning ? "success" : "warning"}>{ollamaRunning ? "READY" : "SETUP"}</StatusPill></div>
                  <p>{ollamaRunning ? `${localSetup?.endpoint ?? "http://localhost:11434"} · ${status?.model.installedModels.length ?? 0} installed model${(status?.model.installedModels.length ?? 0) === 1 ? "" : "s"}` : localSetup?.setupMessage ?? "Check the local Ollama service before using a local model."}</p>
                  {!ollamaInstalled ? <button className="primary-action" type="button" onClick={() => void openOllamaInstall()}>{localSetup?.ollamaInstallAction?.label ?? "Install Ollama"}</button> : <button className="secondary-action" type="button" onClick={() => void refreshStatus()}>Refresh status</button>}
                </aside>
              </div>
              {selectedModelCard && (
                <aside className={`memory-fit current-model-fit memory-fit--${currentModelFit?.fit ?? "unknown"}`}>
                  <div><StatusPill tone={modelFitTone(currentModelFit)}>{modelFitLabel(currentModelFit).toUpperCase()}</StatusPill><strong>Current model · <code>{selectedModelCard.id}</code> · 32K</strong></div>
                  {currentModelFit?.fit === "not_recommended" ? <p><strong>Current model is not recommended for {primaryFitLimitLabel} {primaryFitLimitGb ? `${primaryFitLimitGb} GB` : "at the detected limit"} at 32K context.</strong> {currentModelFit.message}</p> : <p>{currentModelFit?.message ?? "Kairos keeps your current model visible and will never switch it automatically."}</p>}
                  <div className="current-model-fit__actions">
                    <button className="secondary-action" type="button" onClick={scrollToRecommendations} disabled={recommendedModelCards.length === 0}>Choose a recommended model</button>
                    {selectedModelCard.installed ? <button className="text-action" type="button" onClick={() => void handleTestModel(selectedModelCard.id)} disabled={Boolean(modelOperation) || planningBusy}>{modelOperation?.kind === "test" && sameModelId(modelOperation.model, selectedModelCard.id) ? "Testing…" : "Test current at 32K"}</button> : <><button className="text-action" type="button" onClick={() => void handlePullModel(selectedModelCard.id)} disabled={Boolean(modelOperation) || planningBusy}>{modelOperation?.kind === "pull" && sameModelId(modelOperation.model, selectedModelCard.id) ? "Downloading…" : "Download current model"}</button><span className="current-model-fit__note">Download the current model before testing it.</span></>}
                    <button className="text-action" type="button" onClick={() => setShowAllModels(true)}>Open full catalog</button>
                  </div>
                </aside>
              )}
              <div className="model-catalog-heading" id="recommended-models"><div><p className="section-label">Recommended local models</p><h3>{memoryBudgetPending ? `Pending shortlist for ${effectiveMemoryBudget} GB` : `Best fit for ${hardwareProfileLabel}`} · 32K context</h3></div><p>Only models that comfortably fit this hardware profile and one 32K conversation appear here. Kairos shows at most four.</p></div>
              {recommendedModelCards.length > 0 ? <div className="model-grid">{recommendedModelCards.map((setupModel) => renderModelCard(setupModel, "recommended"))}</div> : <p className="model-catalog-empty">No additional model is recommended for this exact plan. Your current model remains unchanged; adjust the plan or open the full catalog to inspect test-required options.</p>}
              <div className="model-catalog-disclosure">
                <div><strong>Advanced catalog</strong><p>Potentially tight, specialist, unsupported, or test-required models stay out of the default shortlist.</p></div>
                <button className="secondary-action" type="button" onClick={() => setShowAllModels((visible) => !visible)} aria-expanded={showAllModels} aria-controls="advanced-model-catalog">{showAllModels ? "Hide all models" : "Show all models"}</button>
              </div>
              {showAllModels && <div id="advanced-model-catalog">{advancedModelCards.length > 0 ? <div className="model-grid model-grid--advanced">{advancedModelCards.map((setupModel) => renderModelCard(setupModel, "advanced"))}</div> : <p className="model-catalog-empty">No additional catalog models are available for this setup.</p>}</div>}
              {setupFeedback && <p className="settings-feedback">{setupFeedback}</p>}
            </section>

            <section className="settings-grid">
              <article className="settings-card">
                <div className="settings-card__heading"><div><p className="section-label">Summon</p><h2>Opening behaviour</h2></div><kbd>⌥ Space</kbd></div>
                <label className="field-label"><span>Shortcut</span><input value={preferences.shortcut} onChange={(event) => setPreferences((current) => ({ ...current, shortcut: event.target.value }))} /></label>
                <div className="provider-boundary"><strong>Always opens compact chat</strong><span>The global shortcut returns to the focused chatbox. Use the Expand control when you want the full brain cockpit.</span></div>
                <label className="toggle-row"><input type="checkbox" checked={preferences.launchAtLogin} onChange={(event) => setPreferences((current) => ({ ...current, launchAtLogin: event.target.checked }))} /><span><strong>Launch at login</strong><small>Keep Kairos ready in the menu bar.</small></span></label>
                <label className="toggle-row"><input type="checkbox" checked={preferences.closeToHide} onChange={(event) => setPreferences((current) => ({ ...current, closeToHide: event.target.checked }))} /><span><strong>Hide when the window closes</strong><small>Keep Kairos running in the menu bar instead of quitting.</small></span></label>
                <label className="toggle-row"><input type="checkbox" checked={preferences.keepAboveOtherWindows} onChange={(event) => setPreferences((current) => ({ ...current, keepAboveOtherWindows: event.target.checked }))} /><span><strong>Keep above other windows</strong><small>Off by default, so Kairos behaves like a normal window on this desktop.</small></span></label>
                <label className="field-label"><span>Context window</span><select value={preferences.contextWindowTokens} onChange={(event) => setPreferences((current) => ({ ...current, contextWindowTokens: Number(event.target.value) }))}><option value={16_384}>16K · lighter local load</option><option value={32_768}>32K · recommended start</option><option value={65_536}>64K · requires more memory</option></select></label>
                <button className="secondary-action" onClick={() => void savePreferences()}>Save preferences</button>
                {preferencesFeedback && <p className="settings-feedback">{preferencesFeedback}</p>}
              </article>

              <article className="settings-card" id="provider-access-settings">
                <div className="settings-card__heading"><div><p className="section-label">Provider</p><h2>Inference destination</h2></div><StatusPill tone={activeProvider.external ? "warning" : "local"}>{activeProvider.external ? "REVIEW" : "LOCAL"}</StatusPill></div>
                <label className="field-label"><span>Active provider</span><select value={providerId} onChange={(event) => setProviderId(event.target.value as ProviderId)}>{providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.label}</option>)}</select></label>
                <p className="provider-detail">{activeProvider.detail}</p>
                {activeProvider.id === "ollama" ? (
                  <label className="field-label"><span>Active local model</span><select value={selectedInstalledOllamaModel} onChange={(event) => void selectModel(event.target.value)} disabled={!status || busy || !nativeRuntime || installedOllamaChoices.length === 0}>
                    {!status && <option value="">Checking installed models…</option>}
                    {status && installedOllamaChoices.length === 0 && <option value="">No installed local model</option>}
                    {status && !selectedInstalledOllamaModel && installedOllamaChoices.length > 0 && <option value="" disabled>Current model is not installed</option>}
                    {installedOllamaChoices.map((choice) => <option key={choice.id} value={choice.id}>{choice.label}</option>)}
                  </select></label>
                ) : (
                  <>
                    <div className="provider-boundary"><strong>Cloud consent boundary</strong><span>Every turn shows the outgoing message, approved notes, temporary-file extraction, destination, and model before it is sent. Keys stay in macOS Keychain when connected natively.</span></div>
                    {activeProvider.id === "openai" || activeProvider.id === "anthropic" ? (
                      <>
                        <label className="field-label"><span>Model identifier</span><input value={providerModel} onChange={(event) => setProviderModel(event.target.value)} placeholder="Choose the model you pay for" /></label>
                        <label className="field-label"><span>API key</span><input type="password" value={providerApiKey} onChange={(event) => setProviderApiKey(event.target.value)} placeholder="Stored only in macOS Keychain" autoComplete="off" /></label>
                      </>
                    ) : null}
                    <button className="secondary-action" onClick={() => void saveProviderSettings()} disabled={providerSettingsBusy || !nativeRuntime}>{providerSettingsBusy ? "Saving…" : "Save provider"}</button>
                    {providerSettingsFeedback && <p className="settings-feedback">{providerSettingsFeedback}</p>}
                  </>
                )}
              </article>
            </section>

            <section className="settings-card brain-registry-card" id="brain-registry-settings">
              <div className="settings-card__heading">
                <div><p className="section-label">Brain registry</p><h2>Connected Obsidian brains</h2><p>Register a folder through the native picker, then explicitly choose its policy.</p></div>
                <button className="primary-action" onClick={() => void pickBrainFolder()}>Add brain</button>
              </div>
              <div className="brain-card-grid">
                {brains.map((brain) => {
                  const visual = brainVisual(brain.id);
                  return <article className="brain-card" key={brain.id} style={{ "--brain-color": visual.color } as CSSProperties}>
                    <div className="brain-card__heading"><span className="brain-card__mark">{visual.label.slice(0, 1)}</span><div><h3>{brain.name}</h3><p>{brain.role}</p></div><StatusPill tone={brain.status === "ready" ? "success" : brain.status === "offline" ? "warning" : "neutral"}>{brain.status ?? "ready"}</StatusPill></div>
                    {brain.rootPath && <code>{brain.rootPath}</code>}
                    <div className="brain-policy-row"><span>{brain.graphEnabled ? "In graph" : "Graph hidden"}</span><span>{brain.egressPolicy?.replaceAll("_", " ")}</span><span>{brain.writePolicy?.replaceAll("_", " ")}</span></div>
                    <div className="brain-card__actions"><button className="text-action" onClick={() => setPolicyEditor((current) => current?.brainId === brain.id ? null : policyDraftFor(brain))}>{policyEditor?.brainId === brain.id ? "Close policy" : "Configure policy"}</button></div>
                    {policyEditor?.brainId === brain.id && (
                      <form className="brain-policy-editor" onSubmit={(event) => { event.preventDefault(); void updateBrainPolicy(); }}>
                        <label className="field-label"><span>Egress</span><select value={policyEditor.egressPolicy} onChange={(event) => setPolicyEditor((current) => current ? { ...current, egressPolicy: event.target.value as BrainPolicyDraft["egressPolicy"] } : current)}><option value="local_only">Local only</option><option value="cloud_allowed">Cloud allowed · preview every turn</option><option value="redact_required">Redaction required</option></select></label>
                        <label className="field-label"><span>Note writes</span><select value={policyEditor.writePolicy} onChange={(event) => setPolicyEditor((current) => current ? { ...current, writePolicy: event.target.value as BrainPolicyDraft["writePolicy"] } : current)}><option value="readonly">Read only</option><option value="confirm_every_write">Confirm every write</option><option value="prohibited">Prohibited</option></select></label>
                        <label className="toggle-row"><input type="checkbox" checked={policyEditor.graphEnabled} onChange={(event) => setPolicyEditor((current) => current ? { ...current, graphEnabled: event.target.checked } : current)} /><span><strong>Include safe metadata in Brain Map</strong><small>Protected and explicit-only notes stay excluded.</small></span></label>
                        <p className="policy-boundary">Cloud allowed still requires a per-turn review. Prohibited disables note writes completely.</p>
                        <div className="consent-actions"><button type="button" className="secondary-action" onClick={() => setPolicyEditor(null)} disabled={policyBusy}>Cancel</button><button className="primary-action" type="submit" disabled={policyBusy}>{policyBusy ? "Saving…" : "Save policy"}</button></div>
                      </form>
                    )}
                  </article>;
                })}
              </div>
              {addBrain && (
                <form className="add-brain-form" onSubmit={(event) => { event.preventDefault(); void createBrain(); }}>
                  <div className="form-heading"><div><p className="section-label">Confirm new brain</p><h3>Review scope before registration</h3><p>{addBrain.displayPath ?? "Native folder selected"}{addBrain.noteCount !== undefined ? ` · ${addBrain.noteCount} safe notes found` : ""}</p></div><button type="button" className="icon-button" onClick={() => setAddBrain(null)} aria-label="Cancel add brain">×</button></div>
                  <div className="retrieval-scope">
                    <p className="section-label">Planned retrieval scope</p>
                    <div><strong>Startup router paths</strong>{addBrain.routerCandidates.length ? <ul>{addBrain.routerCandidates.map((path) => <li key={path}><code>{path}</code></li>)}</ul> : <span>None found — this brain will not receive startup router context.</span>}</div>
                    <div><strong>Context retrieval paths</strong>{addBrain.contextCandidates.length ? <ul>{addBrain.contextCandidates.map((path) => <li key={path}><code>{path}</code></li>)}</ul> : <span>None found — no notes are planned for retrieval until configured.</span>}</div>
                  </div>
                  <div className="form-grid">
                    <label className="field-label"><span>Name</span><input required value={addBrain.name} onChange={(event) => setAddBrain((current) => current ? { ...current, name: event.target.value } : current)} /></label>
                    <label className="field-label"><span>Role</span><input required value={addBrain.role} onChange={(event) => setAddBrain((current) => current ? { ...current, role: event.target.value } : current)} /></label>
                    <label className="field-label form-span"><span>Routing hints (comma-separated)</span><input value={addBrain.routingHints} onChange={(event) => setAddBrain((current) => current ? { ...current, routingHints: event.target.value } : current)} placeholder="life, planning, personal" /></label>
                    <label className="field-label"><span>Egress</span><select value={addBrain.egressPolicy} onChange={(event) => setAddBrain((current) => current ? { ...current, egressPolicy: event.target.value } : current)}><option value="local_only">Local only</option><option value="cloud_allowed">Cloud allowed · preview every turn</option></select></label>
                    <label className="field-label"><span>Writes</span><select value={addBrain.writePolicy} onChange={(event) => setAddBrain((current) => current ? { ...current, writePolicy: event.target.value } : current)}><option value="confirm_every_write">Confirm every write</option><option value="readonly">Read only</option></select></label>
                  </div>
                  <label className="toggle-row"><input type="checkbox" checked={addBrain.graphEnabled} onChange={(event) => setAddBrain((current) => current ? { ...current, graphEnabled: event.target.checked } : current)} /><span><strong>Include safe metadata in Brain Map</strong><small>Private and explicit-only notes stay excluded.</small></span></label>
                  <div className="consent-actions"><button type="button" className="secondary-action" onClick={() => setAddBrain(null)}>Cancel</button><button className="primary-action" type="submit">Register brain</button></div>
                </form>
              )}
            </section>

            <section className="settings-card write-surface" id="confirmed-write-settings">
              <div className="settings-card__heading"><div><p className="section-label">Confirmed note update</p><h2>Draft, review, then write.</h2><p>Kairos only creates or edits Markdown inside an explicitly enabled brain directory. It never deletes or moves notes.</p></div><StatusPill tone="warning">CONFIRM</StatusPill></div>
              {!writeProposal ? (
                writableBrains.length === 0 ? (
                  <div className="write-policy-empty" role="status"><strong>No writable brain is enabled.</strong><span>Notes stay safe: set a registered brain to <em>Confirm every write</em> in its policy before Kairos can prepare a bounded diff.</span></div>
                ) : (
                <div className="form-grid write-form">
                  <label className="field-label"><span>Brain</span><select value={writeDraft.brainId} onChange={(event) => setWriteDraft((current) => ({ ...current, brainId: event.target.value }))}>{writableBrains.map((brain) => <option key={brain.id} value={brain.id}>{brain.name}</option>)}</select></label>
                  <label className="field-label"><span>Operation</span><select value={writeDraft.kind} onChange={(event) => setWriteDraft((current) => ({ ...current, kind: event.target.value as "create" | "edit" }))}><option value="create">Create note</option><option value="edit">Edit note</option></select></label>
                  <label className="field-label form-span"><span>Relative Markdown path</span><input value={writeDraft.relativePath} onChange={(event) => setWriteDraft((current) => ({ ...current, relativePath: event.target.value }))} placeholder="02_Projects/Kairos/Capture.md" /></label>
                  <label className="field-label form-span"><span>Proposed Markdown</span><textarea value={writeDraft.markdown} onChange={(event) => setWriteDraft((current) => ({ ...current, markdown: event.target.value }))} rows={7} placeholder="# Note title&#10;&#10;Your approved update…" /></label>
                  <div className="consent-actions form-span"><button className="secondary-action" onClick={() => void previewNoteWrite()} disabled={writeBusy || !writeDraft.markdown.trim() || !nativeRuntime}>{writeBusy ? "Preparing…" : "Preview diff"}</button></div>
                </div>
                )
              ) : (
                <div className="write-preview">
                  <div className="provider-boundary"><strong>{writeProposal.kind === "create" ? "Create" : "Edit"} {writeProposal.relativePath}</strong><span>Review this bounded diff carefully. It expires at {shortDate(writeProposal.expiresAt) ?? "soon"}; confirmation rechecks for conflicts.</span></div>
                  <pre className="write-diff">{writeProposal.diff}</pre>
                  <div className="consent-actions"><button className="secondary-action" onClick={() => setWriteProposal(null)} disabled={writeBusy}>Cancel</button><button className="primary-action" onClick={() => void confirmNoteWrite()} disabled={writeBusy}>{writeBusy ? "Saving…" : "Confirm write"}</button></div>
                </div>
              )}
              {writeFeedback && <p className="settings-feedback">{writeFeedback}</p>}
            </section>
          </section>
        )}
      </section>
    </main>
  );
}
