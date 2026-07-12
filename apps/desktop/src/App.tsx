import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

const briefQuestion = "What should I do next, and why?";

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [pack, setPack] = useState<ContextPack | null>(null);
  const [answer, setAnswer] = useState<BriefAnswer | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const modelReady = Boolean(status?.model.running && status.model.selectedModelInstalled);

  const refreshStatus = async () => {
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
    <main>
      <header>
        <div>
          <p className="eyebrow">KAIROS · LOCAL-ONLY DEVELOPER ALPHA</p>
          <h1>The right context,<br />at the right moment.</h1>
        </div>
        <kbd>⌥ Space</kbd>
      </header>

      <section className="hero">
        <p>{briefQuestion}</p>
        {!status?.initialized ? (
          <button onClick={initialize} disabled={busy}>
            {busy ? "Preparing…" : "Create my local Tharm registry"}
          </button>
        ) : (
          <button onClick={brief} disabled={busy || !modelReady}>
            {busy ? "Composing…" : "Compose my brief"}
          </button>
        )}
        {status?.initialized && (
          <div className="model-settings">
            <label htmlFor="local-model">Local model</label>
            <select
              id="local-model"
              value={status.model.selectedModel}
              disabled={busy}
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
          </div>
        )}
        <small>
          {status?.initialized
            ? modelReady
              ? `${status.model.resolvedModel ?? status.model.selectedModel} is ready. No cloud or automatic model fallback is enabled.`
              : status.model.setupMessage ?? "The selected local model is not ready yet."
            : "This developer alpha creates a local registry only. It does not copy or modify your notes."}
        </small>
      </section>

      {error && <p className="error">{error}</p>}

      {pack && (
        <section className="result">
          <div className="route">
            {pack.route.brains.map((brain) => (
              <span key={brain.id}>{brain.name}</span>
            ))}
          </div>
          <h2>Grounded context ready</h2>
          <p className="muted">
            Kairos selected the local sources below. Its built-in Ollama path can
            synthesize from them locally; MCP is route-only in this alpha and
            never transfers note text.
          </p>

          {answer && (
            <article className="answer">
              <p className="eyebrow">LOCAL {status?.model.resolvedModel ?? status?.model.selectedModel}</p>
              <h3>{answer.action}</h3>
              <p>{answer.why}</p>
              <p className="muted">{answer.caveat}</p>
              {answer.sourceIds.length > 0 && (
                <p className="citation">
                  Grounded in {answer.sourceIds.map((id) => <code key={id}>{id}</code>)}
                </p>
              )}
            </article>
          )}

          {pack.freshnessWarnings.length > 0 && (
            <aside>
              {pack.freshnessWarnings.map((warning) => <p key={warning}>{warning}</p>)}
            </aside>
          )}

          <div className="sources">
            {pack.sources.map(({ source, content, truncated }) => (
              <details key={source.id}>
                <summary>
                  <span>{source.brainId}</span>
                  {source.relativePath}
                </summary>
                <pre>{content}</pre>
                {truncated && <small>Source was bounded before it reached a model.</small>}
              </details>
            ))}
          </div>

          {pack.withheldSources.length > 0 && (
            <p className="muted">Protected sources were withheld by policy.</p>
          )}
        </section>
      )}
    </main>
  );
}
