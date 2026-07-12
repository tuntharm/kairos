import { type CSSProperties, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import brandMark from "../../../design/assets/brand/kairos-mark-gradient.svg";
import brandWordmark from "../../../design/assets/brand/kairos-wordmark-dark.svg";
import borderFiberCitation from "../../../design/assets/citations/border-fiber.svg";
import datterCitation from "../../../design/assets/citations/datter.svg";
import everydayCitation from "../../../design/assets/citations/everyday.svg";
import fileCitation from "../../../design/assets/citations/file.svg";
import phdCitation from "../../../design/assets/citations/phd.svg";
import routeError from "../../../design/assets/routing/error.svg";
import routeIdle from "../../../design/assets/routing/idle.svg";
import routeMulti from "../../../design/assets/routing/routed-multi.svg";
import routeOne from "../../../design/assets/routing/routed-one.svg";

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
    choices: Array<{ id: string; label: string; role: string }>;
  };
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

type BrainVisual = {
  label: string;
  color: string;
  icon: string;
};

const briefQuestion = "What should I do next, and why?";

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
      { id: "qwen3:8b", label: "Qwen 3 8B", role: "Fast router / manual fallback" },
      { id: "gpt-oss:20b", label: "GPT-OSS 20B", role: "Optional alternative" },
      { id: "glm-4.7-flash", label: "GLM 4.7 Flash", role: "Optional alternative" },
    ],
  },
};

function hasNativeBridge() {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const brainVisuals: Record<string, BrainVisual> = {
  everyday: { label: "Everyday", color: "#4C9FFF", icon: everydayCitation },
  phd: { label: "PhD", color: "#9E7BFF", icon: phdCitation },
  datter: { label: "Datter", color: "#49D7D2", icon: datterCitation },
  "border-fiber": { label: "Border Fiber", color: "#FFB85C", icon: borderFiberCitation },
};

const fileVisual: BrainVisual = { label: "File", color: "#A5B4CC", icon: fileCitation };

function brainVisual(brainId: string): BrainVisual {
  return brainVisuals[brainId] ?? fileVisual;
}

function compactPath(path: string) {
  const pieces = path.split("/");
  return pieces.length > 2 ? pieces.slice(-2).join("/") : path;
}

function CitationChip({ source }: { source: Source["source"] }) {
  const visual = brainVisual(source.brainId);
  return (
    <span
      className="citation-chip"
      style={{ "--chip-color": visual.color } as CSSProperties}
      title={source.relativePath}
    >
      <img src={visual.icon} alt="" aria-hidden="true" />
      <span>{visual.label}</span>
      <span className="citation-chip__detail">{compactPath(source.relativePath)}</span>
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

export default function App() {
  const nativeRuntime = hasNativeBridge();
  const [status, setStatus] = useState<AppStatus | null>(() => (
    nativeRuntime ? null : browserPreviewStatus
  ));
  const [pack, setPack] = useState<ContextPack | null>(null);
  const [answer, setAnswer] = useState<BriefAnswer | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const modelReady = Boolean(status?.model.running && status.model.selectedModelInstalled);
  const activeModel = status?.model.resolvedModel ?? status?.model.selectedModel ?? "Checking Ollama";
  const routeIcon = error
    ? routeError
    : pack
      ? pack.route.brains.length > 1
        ? routeMulti
        : routeOne
      : routeIdle;
  const routeLabel = busy
    ? "Routing local evidence"
    : error
      ? "Needs attention"
      : pack
        ? pack.route.brains.length > 1
          ? "Composed route"
          : "Authoritative route"
        : "Ready to route";

  const refreshStatus = async () => {
    if (!nativeRuntime) {
      setStatus(browserPreviewStatus);
      return;
    }
    try {
      setStatus(await invoke<AppStatus>("app_status"));
    } catch (reason) {
      setError(String(reason));
    }
  };

  useEffect(() => {
    void refreshStatus();
  }, []);

  const initialize = async () => {
    if (!nativeRuntime) return;
    setBusy(true);
    setError(null);
    try {
      setStatus(await invoke<AppStatus>("initialize_tharm_profile"));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const brief = async () => {
    if (!nativeRuntime) return;
    setBusy(true);
    setError(null);
    try {
      const current = await invoke<AppStatus>("app_status");
      setStatus(current);
      if (!current.model.running || !current.model.selectedModelInstalled) {
        setAnswer(null);
        setPack(null);
        setError(current.model.setupMessage ?? "The selected local model is not ready yet.");
        return;
      }
      const result = await invoke<LocalBrief>("brief_with_ollama");
      setAnswer(result.answer);
      setPack(result.context);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  const selectModel = async (model: string) => {
    if (!nativeRuntime) return;
    setBusy(true);
    setError(null);
    try {
      setStatus(await invoke<AppStatus>("set_selected_model", { model }));
      setAnswer(null);
      setPack(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="app-shell">
      <section className="control-plane">
        <header className="app-header">
          <div className="brand-lockup">
            <img className="brand-mark" src={brandMark} alt="Kairos" />
            <img className="brand-wordmark" src={brandWordmark} alt="Kairos — Knowledge, Action, Intelligence, Routing" />
          </div>
          <div className="summon-key" title="Toggle Kairos">
            <span>SUMMON</span>
            <kbd>⌥ Space</kbd>
          </div>
        </header>

        <section className="moment-panel" aria-labelledby="brief-question">
          <div className="routing-visual" aria-hidden={busy}>
            {busy ? <KairosLoader /> : <img src={routeIcon} alt="" />}
          </div>
          <div className="moment-copy">
            <p className="section-label">{routeLabel}</p>
            <h1 id="brief-question">{briefQuestion}</h1>
            <p className="moment-description">
              Local evidence, deliberate routing, one useful next action.
            </p>
          </div>

          <div className="runtime-status" aria-live="polite">
            <span className={`status-dot ${modelReady ? "is-ready" : "needs-setup"}`} />
            <span>
              {status?.initialized
                ? modelReady
                  ? `${activeModel} · local`
                  : status.model.setupMessage ?? "Local model needs setup"
                : "Create a local registry to connect your brains"}
            </span>
          </div>

          {!nativeRuntime && (
            <p className="preview-notice">
              Browser preview · local vault access is available in the Kairos desktop app.
            </p>
          )}

          {!status?.initialized ? (
            <button
              className="primary-action"
              onClick={initialize}
              disabled={busy || !nativeRuntime}
              title={nativeRuntime ? undefined : "Open the desktop app to connect your local brains"}
            >
              {busy ? "Preparing local registry…" : "Connect my existing brains"}
            </button>
          ) : (
            <button
              className="primary-action"
              onClick={brief}
              disabled={busy || !modelReady || !nativeRuntime}
              title={nativeRuntime ? undefined : "Open the desktop app to compose a local brief"}
            >
              {busy ? "Composing from local evidence…" : "Compose my brief"}
            </button>
          )}

          {status?.initialized && (
            <label className="model-settings" htmlFor="local-model">
              <span>Local synthesis model</span>
              <select
                id="local-model"
                value={status.model.selectedModel}
                disabled={busy || !nativeRuntime}
                onChange={(event) => void selectModel(event.target.value)}
              >
                {status.model.choices.map((choice) => (
                  <option key={choice.id} value={choice.id}>
                    {choice.label} — {choice.role}
                  </option>
                ))}
              </select>
              <small>
                Saved locally · {status.model.contextWindowTokens / 1024}K context · {status.model.endpoint}
              </small>
            </label>
          )}
        </section>

        {error && <p className="error-state" role="alert">{error}</p>}

        {pack && (
          <section className="brief-result" aria-live="polite">
            <div className="result-header">
              <div className="route-summary">
                <img src={routeIcon} alt="" />
                <div>
                  <p className="section-label">{routeLabel}</p>
                  <h2>{pack.route.brains.map((brain) => brain.name).join(" + ")}</h2>
                </div>
              </div>
              <span className="local-boundary">LOCAL ONLY</span>
            </div>

            {answer && (
              <article className="selected-action">
                <p className="section-label">Selected moment · {activeModel}</p>
                <h3>{answer.action}</h3>
                <p>{answer.why}</p>
                <p className="action-caveat">{answer.caveat}</p>
                {answer.sourceIds.length > 0 && (
                  <div className="citation-row" aria-label="Grounding sources">
                    {pack.sources
                      .filter((source) => answer.sourceIds.includes(source.source.id))
                      .slice(0, 3)
                      .map(({ source }) => <CitationChip key={source.id} source={source} />)}
                    {answer.sourceIds.length > 3 && (
                      <span className="citation-more">+{answer.sourceIds.length - 3}</span>
                    )}
                  </div>
                )}
              </article>
            )}

            {pack.freshnessWarnings.length > 0 && (
              <aside className="freshness-notice">
                {pack.freshnessWarnings.map((warning) => <p key={warning}>{warning}</p>)}
              </aside>
            )}

            <details className="evidence-drawer">
              <summary>
                <span>Local evidence</span>
                <span>{pack.sources.length} approved source{pack.sources.length === 1 ? "" : "s"}</span>
              </summary>
              <div className="evidence-list">
                {pack.sources.map(({ source, content, truncated }) => (
                  <article className="evidence-item" key={source.id}>
                    <div className="evidence-item__meta">
                      <CitationChip source={source} />
                      {source.modifiedAt && <time>{new Date(source.modifiedAt).toLocaleDateString()}</time>}
                    </div>
                    <pre>{content}</pre>
                    {truncated && <small>Bounded before reaching the local model.</small>}
                  </article>
                ))}
              </div>
            </details>

            {pack.withheldSources.length > 0 && (
              <p className="withheld-note">Protected sources were withheld by policy.</p>
            )}
          </section>
        )}
      </section>
    </main>
  );
}
