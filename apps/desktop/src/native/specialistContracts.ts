export type SpecialistId = string;
export type DatasetId = string;
export type RunId = string;
export type CandidateId = string;
export type ReleaseId = string;

export type DatasetState = "draft" | "reviewed" | "frozen" | "superseded";
export type SpecialistJobState =
  | "queued"
  | "preflight"
  | "running"
  | "validating"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "interrupted";
export type MeasuredMetricV1 = {
  name: string;
  value: number | null;
  unit: string | null;
};

export type SpecialistDatasetSummaryV1 = {
  schemaVersion: 1;
  datasetId: DatasetId;
  state: DatasetState;
  caseCount: number | null;
  scenarioGroupCount: number | null;
  sha256: string | null;
  createdAt: string;
  frozenAt: string | null;
};

export type SpecialistTestSummaryV1 = {
  schemaVersion: 1;
  testSuiteId: string;
  name: string;
  caseCount: number | null;
  sealed: boolean;
  sha256: string | null;
};

export type SpecialistRunSummaryV1 = {
  schemaVersion: 1;
  runId: RunId;
  kind: "synthetic_smoke" | "baseline" | "training" | "evaluation";
  state: SpecialistJobState;
  datasetId: DatasetId | null;
  candidateId: CandidateId | null;
  metrics: MeasuredMetricV1[];
  safeMessage: string | null;
  createdAt: string;
  updatedAt: string;
};

export type SpecialistComparisonV1 = {
  schemaVersion: 1;
  comparisonId: string;
  baselineRunId: RunId;
  candidateRunId: RunId;
  candidateId: CandidateId;
  verdict: "eligible" | "not_eligible" | "inconclusive";
  metrics: MeasuredMetricV1[];
  createdAt: string;
};

export type ReleaseManifestV1 = {
  schemaVersion: 1;
  specialistId: SpecialistId;
  releaseId: ReleaseId;
  baseSha256: string;
  adapterSha256: string | null;
  instructionsSha256: string;
  sourceSha256: string;
  datasetSha256: string;
  toolSha256: string;
  evaluationId: string;
  evaluationSha256: string;
  evaluatedBoundarySha256: string;
  readinessVerdict: "eligible" | "not_eligible" | "inconclusive";
  failureReason: string | null;
  activeReleaseId: ReleaseId | null;
  previousReleaseId: ReleaseId | null;
  createdAt: string;
  activatedAt: string | null;
};

export type SpecialistSummaryV1 = {
  schemaVersion: 1;
  specialistId: SpecialistId;
  name: string;
  purpose: string;
  activeReleaseId: ReleaseId | null;
  previousReleaseId: ReleaseId | null;
};

export type SpecialistDetailV1 = SpecialistSummaryV1 & {
  datasets: SpecialistDatasetSummaryV1[];
  tests: SpecialistTestSummaryV1[];
  runs: SpecialistRunSummaryV1[];
  comparisons: SpecialistComparisonV1[];
  releases: ReleaseManifestV1[];
};

export type SpecialistDraftRequestV1 = {
  schemaVersion: 1;
  specialistId: SpecialistId;
  name: string;
  purpose: string;
};

export type SpecialistExecutionRequestV1 = {
  schemaVersion: 1;
  specialistId: SpecialistId;
  input: string;
  sessionId?: string;
};

export type ExperimentContractCheckV1 = {
  presentFields: string[];
  missingFields: string[];
  conflictingFields: string[];
};

export type ExperimentReviewV1 = {
  assumptions: string[];
  decisionStatus: "accepted" | "working_hypothesis" | "rejected" | "deferred" | "needs_supervision";
  supportedFindings: Array<{ claim: string; evidenceIds: string[] }>;
  unsupportedClaims: Array<{ claim: string; evidenceIds: string[] }>;
  missingEvidence: string[];
  checkerResult: ExperimentContractCheckV1;
  nextExperiment: {
    changedVariable: string;
    fixedControls: string[];
    metric: string;
    decisionRule: string;
    stopCondition: string;
  };
  citations: Array<{ evidenceId: string; claim: string }>;
};

export type SpecialistResponseIdentityV1 = {
  schemaVersion: 1;
  specialistId: SpecialistId;
  specialistName: string;
  releaseId: ReleaseId;
  releaseSha256: string;
  evaluationSha256: string;
  evidenceBoundarySha256: string;
};

export type SpecialistExecutionV1 = {
  schemaVersion: 1;
  output: ExperimentReviewV1;
  identity: SpecialistResponseIdentityV1;
};
