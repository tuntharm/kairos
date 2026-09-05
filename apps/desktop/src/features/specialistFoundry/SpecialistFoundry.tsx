import { type ReactNode, useEffect, useMemo, useState } from "react";
import {
  createSpecialistClient,
  SpecialistNativeError,
  type SpecialistClient,
} from "../../native/specialistClient";
import type {
  MeasuredMetricV1,
  ReleaseManifestV1,
  SpecialistDetailV1,
  SpecialistSummaryV1,
} from "../../native/specialistContracts";

export type SpecialistFoundryTab = "specialists" | "data" | "runs" | "compare" | "releases";

export type SpecialistFoundryState =
  | { status: "loading" }
  | { status: "unavailable"; message: string }
  | { status: "empty" }
  | { status: "failed"; message: string }
  | { status: "ready"; specialists: SpecialistSummaryV1[]; selected: SpecialistDetailV1 };

type ReleaseActionState =
  | { status: "idle" }
  | { status: "working"; message: string }
  | { status: "succeeded"; message: string }
  | { status: "failed"; message: string };

export type SpecialistReadClient = Pick<SpecialistClient, "listSpecialists" | "getSpecialist">;

const tabs: Array<{ id: SpecialistFoundryTab; label: string }> = [
  { id: "specialists", label: "Specialists" },
  { id: "data", label: "Data & Tests" },
  { id: "runs", label: "Runs" },
  { id: "compare", label: "Compare" },
  { id: "releases", label: "Releases" },
];

export function formatMeasuredMetric(metric: MeasuredMetricV1): string {
  if (metric.value === null) return "Not measured";
  return `${metric.value}${metric.unit ? ` ${metric.unit}` : ""}`;
}

function formatNullable(value: number | null, suffix = ""): string {
  return value === null ? "Not measured" : `${value}${suffix}`;
}

function failureMessage(reason: unknown): SpecialistFoundryState {
  if (reason instanceof SpecialistNativeError && reason.kind === "unavailable") {
    return {
      status: "unavailable",
      message: "Specialist Lab commands are unavailable in this native build. Existing model setup remains available below.",
    };
  }
  return {
    status: "failed",
    message: reason instanceof Error ? reason.message : "Lab state could not be read.",
  };
}

export async function loadSpecialistFoundryState(client: SpecialistReadClient): Promise<SpecialistFoundryState> {
  try {
    const specialists = await client.listSpecialists();
    if (specialists.length === 0) return { status: "empty" };
    const selected = await client.getSpecialist(specialists[0].specialistId);
    return { status: "ready", specialists, selected };
  } catch (reason) {
    return failureMessage(reason);
  }
}

function StateNotice({ state, onCreateDraft, createBusy }: {
  state: Exclude<SpecialistFoundryState, { status: "ready" }>;
  onCreateDraft?: () => void;
  createBusy?: boolean;
}) {
  if (state.status === "loading") {
    return <div className="foundry-state" role="status"><strong>Loading specialist state…</strong><span>Reading private local manifests through Kairos.</span></div>;
  }
  if (state.status === "empty") {
    return <div className="foundry-state" role="status"><strong>No specialists yet.</strong><span>Create the first private draft and validate Kairos’s 12 public synthetic fixtures. This does not read a brain, run a model, download anything, or train.</span>{onCreateDraft && <button className="primary-action" type="button" disabled={createBusy} onClick={onCreateDraft}>{createBusy ? "Creating local draft…" : "Create Surrogate Reviewer draft"}</button>}</div>;
  }
  return <div className={`foundry-state foundry-state--${state.status}`} role={state.status === "failed" ? "alert" : "status"}><strong>{state.status === "unavailable" ? "Specialist service unavailable" : "Specialist state failed to load"}</strong><span>{state.message}</span></div>;
}

function MetricList({ metrics }: { metrics: MeasuredMetricV1[] }) {
  if (metrics.length === 0) return <p className="foundry-empty-copy">No measured metrics are recorded.</p>;
  return <dl className="foundry-metrics">{metrics.map((metric) => <div key={metric.name}><dt>{metric.name}</dt><dd>{formatMeasuredMetric(metric)}</dd></div>)}</dl>;
}

function SpecialistOverview({ specialists, selected }: { specialists: SpecialistSummaryV1[]; selected: SpecialistDetailV1 }) {
  return (
    <div className="foundry-specialist-grid">
      {specialists.map((specialist) => (
        <article className={`foundry-card ${specialist.specialistId === selected.specialistId ? "is-selected" : ""}`} key={specialist.specialistId}>
          <p className="section-label">Specialist</p>
          <h3>{specialist.name}</h3>
          <p>{specialist.purpose}</p>
          <dl>
            <div><dt>ID</dt><dd><code>{specialist.specialistId}</code></dd></div>
            <div><dt>Active release</dt><dd>{specialist.activeReleaseId ? <code>{specialist.activeReleaseId}</code> : "None active"}</dd></div>
            <div><dt>Previous release</dt><dd>{specialist.previousReleaseId ? <code>{specialist.previousReleaseId}</code> : "None recorded"}</dd></div>
          </dl>
        </article>
      ))}
    </div>
  );
}

function DataAndTests({ specialist }: { specialist: SpecialistDetailV1 }) {
  if (specialist.datasets.length === 0 && specialist.tests.length === 0) {
    return <div className="foundry-tab-empty"><strong>No frozen data or tests.</strong><span>Dataset and sealed-test evidence appears here only after the Lab records it.</span></div>;
  }
  return (
    <div className="foundry-two-column">
      <section><p className="section-label">Datasets</p>{specialist.datasets.length === 0 ? <p className="foundry-empty-copy">No datasets recorded.</p> : specialist.datasets.map((dataset) => <article className="foundry-card" key={dataset.datasetId}><div className="foundry-card__heading"><h3>{dataset.datasetId}</h3><span className="foundry-state-chip">{dataset.state.replaceAll("_", " ")}</span></div><dl><div><dt>Cases</dt><dd>{formatNullable(dataset.caseCount)}</dd></div><div><dt>Scenario groups</dt><dd>{formatNullable(dataset.scenarioGroupCount)}</dd></div><div><dt>Digest</dt><dd>{dataset.sha256 ? <code>{dataset.sha256}</code> : "Not recorded"}</dd></div></dl></article>)}</section>
      <section><p className="section-label">Tests</p>{specialist.tests.length === 0 ? <p className="foundry-empty-copy">No test suites recorded.</p> : specialist.tests.map((test) => <article className="foundry-card" key={test.testSuiteId}><div className="foundry-card__heading"><h3>{test.name}</h3><span className="foundry-state-chip">{test.sealed ? "sealed" : "not sealed"}</span></div><dl><div><dt>Cases</dt><dd>{formatNullable(test.caseCount)}</dd></div><div><dt>Digest</dt><dd>{test.sha256 ? <code>{test.sha256}</code> : "Not recorded"}</dd></div></dl></article>)}</section>
    </div>
  );
}

function Runs({ specialist }: { specialist: SpecialistDetailV1 }) {
  if (specialist.runs.length === 0) return <div className="foundry-tab-empty"><strong>No runs recorded.</strong><span>Queued, active, completed, failed, cancelled, and interrupted jobs will remain explicit here.</span></div>;
  return <div className="foundry-list">{specialist.runs.map((run) => <article className="foundry-card" key={run.runId}><div className="foundry-card__heading"><div><p className="section-label">{run.kind}</p><h3>{run.runId}</h3></div><span className="foundry-state-chip">{run.state}</span></div>{run.safeMessage && <p>{run.safeMessage}</p>}<MetricList metrics={run.metrics} /></article>)}</div>;
}

function Compare({ specialist }: { specialist: SpecialistDetailV1 }) {
  if (specialist.comparisons.length === 0) return <div className="foundry-tab-empty"><strong>No evaluated comparison.</strong><span>Kairos will not show a winner until a frozen baseline and candidate have a recorded evaluation.</span></div>;
  return <div className="foundry-list">{specialist.comparisons.map((comparison) => <article className="foundry-card" key={comparison.comparisonId}><div className="foundry-card__heading"><div><p className="section-label">Candidate {comparison.candidateId}</p><h3>{comparison.baselineRunId} → {comparison.candidateRunId}</h3></div><span className="foundry-state-chip">{comparison.verdict.replaceAll("_", " ")}</span></div><MetricList metrics={comparison.metrics} /></article>)}</div>;
}

function ReleaseEvidence({ release, actionBusy, onActivateRelease }: {
  release: ReleaseManifestV1;
  actionBusy: boolean;
  onActivateRelease?: (releaseId: string) => void;
}) {
  const digests = [
    ["Base model", release.baseSha256],
    ["Adapter", release.adapterSha256],
    ["Instructions", release.instructionsSha256],
    ["Source", release.sourceSha256],
    ["Dataset", release.datasetSha256],
    ["Tool", release.toolSha256],
    ["Evaluation", release.evaluationSha256],
    ["Evidence boundary", release.evaluatedBoundarySha256],
  ];
  return (
    <article className="foundry-card foundry-release-card">
      <div className="foundry-card__heading"><div><p className="section-label">Release evidence</p><h3>{release.releaseId}</h3></div><span className="foundry-state-chip">{release.activeReleaseId === release.releaseId ? "active" : release.readinessVerdict === "eligible" ? "approval ready" : release.readinessVerdict.replaceAll("_", " ")}</span></div>
      <dl className="foundry-release-summary">
        <div><dt>Readiness</dt><dd>{release.readinessVerdict.replaceAll("_", " ")}</dd></div>
        <div><dt>Active</dt><dd>{release.activeReleaseId === release.releaseId ? "Yes" : "No"}</dd></div>
        <div><dt>Previous release</dt><dd>{release.previousReleaseId ? <code>{release.previousReleaseId}</code> : "None recorded"}</dd></div>
        <div><dt>Activated</dt><dd>{release.activatedAt ?? "Not activated"}</dd></div>
      </dl>
      {release.failureReason && <p className="foundry-failure-reason"><strong>Gate failure:</strong> {release.failureReason}</p>}
      <details><summary>Exact evidence identities</summary><dl className="foundry-digests">{digests.map(([label, digest]) => <div key={label}><dt>{label}</dt><dd>{digest ? <code>{digest}</code> : "Not recorded"}</dd></div>)}</dl></details>
      {release.readinessVerdict === "eligible" && release.activeReleaseId !== release.releaseId && onActivateRelease && <div className="foundry-release-actions"><button className="primary-action" type="button" disabled={actionBusy} onClick={() => onActivateRelease(release.releaseId)}>Activate {release.releaseId}</button><small>Native validation rechecks eligibility and the expected active release before the pointer changes.</small></div>}
    </article>
  );
}

function Releases({ specialist, actionState, onActivateRelease, onRollbackRelease }: {
  specialist: SpecialistDetailV1;
  actionState: ReleaseActionState;
  onActivateRelease?: (releaseId: string) => void;
  onRollbackRelease?: () => void;
}) {
  const actionBusy = actionState.status === "working";
  return <div className="foundry-list">
    {specialist.activeReleaseId && specialist.previousReleaseId && onRollbackRelease && <aside className="foundry-rollback"><div><p className="section-label">Active registry</p><strong><code>{specialist.activeReleaseId}</code> is active</strong><span>Previous eligible release: <code>{specialist.previousReleaseId}</code></span></div><button className="secondary-action" type="button" disabled={actionBusy} onClick={onRollbackRelease}>Roll back to {specialist.previousReleaseId}</button></aside>}
    {actionState.status !== "idle" && <p className={`foundry-action-state foundry-action-state--${actionState.status}`} role={actionState.status === "failed" ? "alert" : "status"}>{actionState.message}</p>}
    {specialist.releases.length === 0 ? <div className="foundry-tab-empty"><strong>No releases recorded.</strong><span>An evaluated candidate is not active until a person explicitly activates that exact eligible release.</span></div> : specialist.releases.map((release) => <ReleaseEvidence key={release.releaseId} release={release} actionBusy={actionBusy} onActivateRelease={onActivateRelease} />)}
  </div>;
}

function ReadyTab({ tab, state, actionState, onActivateRelease, onRollbackRelease }: {
  tab: SpecialistFoundryTab;
  state: Extract<SpecialistFoundryState, { status: "ready" }>;
  actionState: ReleaseActionState;
  onActivateRelease?: (releaseId: string) => void;
  onRollbackRelease?: () => void;
}) {
  if (tab === "specialists") return <SpecialistOverview specialists={state.specialists} selected={state.selected} />;
  if (tab === "data") return <DataAndTests specialist={state.selected} />;
  if (tab === "runs") return <Runs specialist={state.selected} />;
  if (tab === "compare") return <Compare specialist={state.selected} />;
  return <Releases specialist={state.selected} actionState={actionState} onActivateRelease={onActivateRelease} onRollbackRelease={onRollbackRelease} />;
}

type SpecialistFoundryViewProps = {
  activeTab: SpecialistFoundryTab;
  onTabChange: (tab: SpecialistFoundryTab) => void;
  state: SpecialistFoundryState;
  runtimeSetup?: ReactNode;
  onRefresh?: () => void;
  releaseActionState?: ReleaseActionState;
  onActivateRelease?: (releaseId: string) => void;
  onRollbackRelease?: () => void;
  onCreateDraft?: () => void;
  createBusy?: boolean;
};

export function SpecialistFoundryView({
  activeTab,
  onTabChange,
  state,
  runtimeSetup,
  onRefresh,
  releaseActionState = { status: "idle" },
  onActivateRelease,
  onRollbackRelease,
  onCreateDraft,
  createBusy,
}: SpecialistFoundryViewProps) {
  return (
    <div className="specialist-foundry">
      <div className="foundry-navigation">
        <div className="foundry-tabs" role="tablist" aria-label="Local AI Lab">
          {tabs.map((tab) => <button key={tab.id} type="button" role="tab" aria-selected={activeTab === tab.id} className={activeTab === tab.id ? "is-active" : ""} onClick={() => onTabChange(tab.id)}>{tab.label}</button>)}
        </div>
        {onRefresh && <button className="text-action foundry-refresh" type="button" onClick={onRefresh} disabled={state.status === "loading"}>Refresh specialists</button>}
      </div>
      <section className="foundry-tab-panel" role="tabpanel" aria-live="polite">
        {state.status === "ready" ? <ReadyTab tab={activeTab} state={state} actionState={releaseActionState} onActivateRelease={onActivateRelease} onRollbackRelease={onRollbackRelease} /> : <StateNotice state={state} onCreateDraft={onCreateDraft} createBusy={createBusy} />}
      </section>
      {activeTab === "specialists" && runtimeSetup && (
        <section className="foundry-runtime-setup" aria-label="Local model setup">
          <div className="foundry-runtime-heading"><p className="section-label">Runtime setup</p><h2>Local model and Ollama</h2><p>Configure the manager runtime here. Specialist release evidence remains separate and never inherits benchmark claims from model setup.</p></div>
          {runtimeSetup}
        </section>
      )}
    </div>
  );
}

type SpecialistFoundryProps = {
  nativeAvailable: boolean;
  runtimeSetup?: ReactNode;
  client?: SpecialistClient;
  onCatalogChanged?: () => void;
};

export function SpecialistFoundry({ nativeAvailable, runtimeSetup, client, onCatalogChanged }: SpecialistFoundryProps) {
  const specialistClient = useMemo(() => client ?? createSpecialistClient(), [client]);
  const [activeTab, setActiveTab] = useState<SpecialistFoundryTab>("specialists");
  const [state, setState] = useState<SpecialistFoundryState>({ status: "loading" });
  const [loadVersion, setLoadVersion] = useState(0);
  const [releaseActionState, setReleaseActionState] = useState<ReleaseActionState>({ status: "idle" });
  const [createBusy, setCreateBusy] = useState(false);

  useEffect(() => {
    let current = true;
    if (!nativeAvailable) {
      setState({ status: "unavailable", message: "Specialists are available in the native Kairos app. Browser preview does not read private Lab state." });
      return () => { current = false; };
    }
    setState({ status: "loading" });
    void loadSpecialistFoundryState(specialistClient).then((nextState) => {
      if (current) setState(nextState);
    });
    return () => { current = false; };
  }, [loadVersion, nativeAvailable, specialistClient]);

  const activateRelease = async (releaseId: string) => {
    if (state.status !== "ready") return;
    const specialistId = state.selected.specialistId;
    if (!window.confirm(`Activate exact release ${releaseId} for ${specialistId}? This atomically changes the release used by Specialist Chat. It does not train, download, or switch the manager model. Kairos will recheck eligibility and current registry state.`)) return;
    setReleaseActionState({ status: "working", message: `Activating ${releaseId}…` });
    try {
      await specialistClient.activateSpecialistRelease(specialistId, releaseId, state.selected.activeReleaseId);
      onCatalogChanged?.();
      setReleaseActionState({ status: "succeeded", message: `${releaseId} was activated and will be re-read from the registry.` });
      setLoadVersion((version) => version + 1);
    } catch (reason) {
      setReleaseActionState({ status: "failed", message: reason instanceof Error ? reason.message : `Activation of ${releaseId} failed.` });
    }
  };

  const rollbackRelease = async () => {
    if (state.status !== "ready" || !state.selected.previousReleaseId) return;
    const { specialistId, activeReleaseId, previousReleaseId } = state.selected;
    if (!window.confirm(`Roll back ${specialistId} from ${activeReleaseId ?? "the active release"} to recorded release ${previousReleaseId}? This atomically changes the release used by Specialist Chat and preserves both release records.`)) return;
    setReleaseActionState({ status: "working", message: `Rolling back to ${previousReleaseId}…` });
    try {
      if (!activeReleaseId) throw new Error("The active release changed. Refresh Specialist Lab before rolling back.");
      await specialistClient.rollbackSpecialistRelease(specialistId, activeReleaseId);
      onCatalogChanged?.();
      setReleaseActionState({ status: "succeeded", message: `Rollback to ${previousReleaseId} completed and will be re-read from the registry.` });
      setLoadVersion((version) => version + 1);
    } catch (reason) {
      setReleaseActionState({ status: "failed", message: reason instanceof Error ? reason.message : `Rollback to ${previousReleaseId} failed.` });
    }
  };

  const createFirstDraft = async () => {
    setCreateBusy(true);
    setReleaseActionState({ status: "idle" });
    try {
      await specialistClient.createSpecialistDraft({
        schemaVersion: 1,
        specialistId: "surrogate-experiment-reviewer",
        name: "Surrogate Experiment Reviewer",
        purpose: "Distinguish supported experiment findings from overclaim, identify missing evidence, and propose one controlled next experiment with citations.",
      });
      onCatalogChanged?.();
      setLoadVersion((version) => version + 1);
    } catch (reason) {
      setState(failureMessage(reason));
    } finally {
      setCreateBusy(false);
    }
  };

  return <SpecialistFoundryView
    activeTab={activeTab}
    onTabChange={setActiveTab}
    state={state}
    runtimeSetup={runtimeSetup}
    onRefresh={() => setLoadVersion((version) => version + 1)}
    releaseActionState={releaseActionState}
    onActivateRelease={(releaseId) => void activateRelease(releaseId)}
    onRollbackRelease={() => void rollbackRelease()}
    onCreateDraft={() => void createFirstDraft()}
    createBusy={createBusy}
  />;
}
