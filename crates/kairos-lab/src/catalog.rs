use crate::{
    JobState, LabError, PrivateLabStore, ReadinessVerdictV1, ReleaseManifestV1, Sha256Digest,
    StableId, synthetic_fixture_cases,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs};

const CATALOG_SCHEMA_VERSION: u16 = 1;
const MAX_CATALOG_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialistRunKindV1 {
    SyntheticSmoke,
    Baseline,
    Training,
    Evaluation,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasuredMetricV1 {
    pub name: String,
    pub value: Option<f64>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistDatasetSummaryV1 {
    pub schema_version: u16,
    pub dataset_id: StableId,
    pub state: crate::DatasetState,
    pub case_count: Option<u32>,
    pub scenario_group_count: Option<u32>,
    pub sha256: Option<Sha256Digest>,
    pub created_at: DateTime<Utc>,
    pub frozen_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistTestSummaryV1 {
    pub schema_version: u16,
    pub test_suite_id: StableId,
    pub name: String,
    pub case_count: Option<u32>,
    pub sealed: bool,
    pub sha256: Option<Sha256Digest>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistRunSummaryV1 {
    pub schema_version: u16,
    pub run_id: StableId,
    pub kind: SpecialistRunKindV1,
    pub state: JobState,
    pub dataset_id: Option<StableId>,
    pub candidate_id: Option<StableId>,
    pub metrics: Vec<MeasuredMetricV1>,
    pub safe_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistComparisonV1 {
    pub schema_version: u16,
    pub comparison_id: StableId,
    pub baseline_run_id: StableId,
    pub candidate_run_id: StableId,
    pub candidate_id: StableId,
    pub verdict: ReadinessVerdictV1,
    pub metrics: Vec<MeasuredMetricV1>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistSummaryV1 {
    pub schema_version: u16,
    pub specialist_id: StableId,
    pub name: String,
    pub purpose: String,
    pub active_release_id: Option<StableId>,
    pub previous_release_id: Option<StableId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistDetailV1 {
    #[serde(flatten)]
    pub summary: SpecialistSummaryV1,
    pub datasets: Vec<SpecialistDatasetSummaryV1>,
    pub tests: Vec<SpecialistTestSummaryV1>,
    pub runs: Vec<SpecialistRunSummaryV1>,
    pub comparisons: Vec<SpecialistComparisonV1>,
    pub releases: Vec<ReleaseManifestV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistDraftRequestV1 {
    pub schema_version: u16,
    pub specialist_id: StableId,
    pub name: String,
    pub purpose: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SpecialistCatalogEntryV1 {
    pub schema_version: u16,
    pub specialist_id: StableId,
    pub name: String,
    pub purpose: String,
    pub datasets: Vec<SpecialistDatasetSummaryV1>,
    pub tests: Vec<SpecialistTestSummaryV1>,
    pub runs: Vec<SpecialistRunSummaryV1>,
    pub comparisons: Vec<SpecialistComparisonV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CatalogV1 {
    pub schema_version: u16,
    pub specialists: BTreeMap<StableId, SpecialistCatalogEntryV1>,
}

impl Default for CatalogV1 {
    fn default() -> Self {
        Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            specialists: BTreeMap::new(),
        }
    }
}

impl PrivateLabStore {
    pub fn list_specialists(&self) -> Result<Vec<SpecialistSummaryV1>, LabError> {
        let catalog = read_catalog_or_default(&self.catalog_path_internal())?;
        catalog
            .specialists
            .values()
            .map(|entry| self.project_summary(entry))
            .collect()
    }

    pub fn specialist_detail(
        &self,
        specialist_id: &StableId,
    ) -> Result<SpecialistDetailV1, LabError> {
        let catalog = read_catalog_or_default(&self.catalog_path_internal())?;
        let entry = catalog
            .specialists
            .get(specialist_id)
            .ok_or(LabError::NotFound)?;
        let summary = self.project_summary(entry)?;
        let releases = match self.release_state(specialist_id) {
            Ok(state) => state.releases.into_values().collect(),
            Err(LabError::NotFound) => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(SpecialistDetailV1 {
            summary,
            datasets: entry.datasets.clone(),
            tests: entry.tests.clone(),
            runs: entry.runs.clone(),
            comparisons: entry.comparisons.clone(),
            releases,
        })
    }

    pub fn create_specialist_draft(
        &mut self,
        request: SpecialistDraftRequestV1,
    ) -> Result<SpecialistDetailV1, LabError> {
        validate_draft_request(&request)?;
        let fixtures = synthetic_fixture_cases()?;
        let fixture_bytes = serde_json::to_vec(&fixtures)?;
        let fixture_sha256 = Sha256Digest::parse(format!("{:x}", Sha256::digest(&fixture_bytes)))?;
        let now = Utc::now();
        let entry = SpecialistCatalogEntryV1 {
            schema_version: 1,
            specialist_id: request.specialist_id.clone(),
            name: request.name,
            purpose: request.purpose,
            datasets: Vec::new(),
            tests: vec![SpecialistTestSummaryV1 {
                schema_version: 1,
                test_suite_id: StableId::parse("public-synthetic-contract-fixtures-v1")?,
                name: "Public synthetic contract fixtures".to_owned(),
                case_count: Some(fixtures.len() as u32),
                sealed: false,
                sha256: Some(fixture_sha256),
            }],
            runs: vec![SpecialistRunSummaryV1 {
                schema_version: 1,
                run_id: StableId::parse("synthetic-contract-smoke-v1")?,
                kind: SpecialistRunKindV1::SyntheticSmoke,
                state: JobState::Succeeded,
                dataset_id: None,
                candidate_id: None,
                metrics: vec![
                    MeasuredMetricV1 {
                        name: "public fixture cases".to_owned(),
                        value: Some(fixtures.len() as f64),
                        unit: Some("cases".to_owned()),
                    },
                    MeasuredMetricV1 {
                        name: "model calls".to_owned(),
                        value: Some(0.0),
                        unit: Some("calls".to_owned()),
                    },
                    MeasuredMetricV1 {
                        name: "training iterations".to_owned(),
                        value: Some(0.0),
                        unit: Some("iterations".to_owned()),
                    },
                ],
                safe_message: Some(
                    "Validated 12 public synthetic fixtures. No model, private source, network, download, or training was used."
                        .to_owned(),
                ),
                created_at: now,
                updated_at: now,
            }],
            comparisons: Vec::new(),
        };

        let _lock = crate::store::RegistryLock::acquire(self.storage_root())?;
        let mut catalog = read_catalog_or_default(&self.catalog_path_internal())?;
        if catalog.specialists.contains_key(&request.specialist_id) {
            return Err(LabError::AlreadyExists);
        }
        catalog
            .specialists
            .insert(request.specialist_id.clone(), entry);
        write_catalog(self, &catalog)?;
        self.specialist_detail(&request.specialist_id)
    }

    pub fn specialist_run(
        &self,
        specialist_id: &StableId,
        run_id: &StableId,
    ) -> Result<SpecialistRunSummaryV1, LabError> {
        self.specialist_detail(specialist_id)?
            .runs
            .into_iter()
            .find(|run| &run.run_id == run_id)
            .ok_or(LabError::NotFound)
    }

    fn project_summary(
        &self,
        entry: &SpecialistCatalogEntryV1,
    ) -> Result<SpecialistSummaryV1, LabError> {
        let release_state = match self.release_state(&entry.specialist_id) {
            Ok(state) => state,
            Err(LabError::NotFound) => Default::default(),
            Err(error) => return Err(error),
        };
        Ok(SpecialistSummaryV1 {
            schema_version: 1,
            specialist_id: entry.specialist_id.clone(),
            name: entry.name.clone(),
            purpose: entry.purpose.clone(),
            active_release_id: release_state.active_release_id,
            previous_release_id: release_state.previous_release_id,
        })
    }

    #[cfg(test)]
    pub(crate) fn catalog_path(&self) -> std::path::PathBuf {
        self.catalog_path_internal()
    }
}

fn validate_draft_request(request: &SpecialistDraftRequestV1) -> Result<(), LabError> {
    if request.schema_version != 1 {
        return Err(LabError::UnsupportedVersion);
    }
    let name = request.name.trim();
    let purpose = request.purpose.trim();
    if name.is_empty()
        || name.chars().count() > 100
        || purpose.is_empty()
        || purpose.chars().count() > 500
    {
        return Err(LabError::InvalidContract(
            "specialist name or purpose is outside the allowed bound",
        ));
    }
    Ok(())
}

fn read_catalog_or_default(path: &std::path::Path) -> Result<CatalogV1, LabError> {
    if !path.exists() {
        return Ok(CatalogV1::default());
    }
    crate::store::reject_symlink(path)?;
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_CATALOG_BYTES {
        return Err(LabError::InputTooLarge);
    }
    let catalog: CatalogV1 = serde_json::from_slice(&fs::read(path)?)?;
    if catalog.schema_version != CATALOG_SCHEMA_VERSION {
        return Err(LabError::UnsupportedVersion);
    }
    Ok(catalog)
}

fn write_catalog(store: &PrivateLabStore, catalog: &CatalogV1) -> Result<(), LabError> {
    let bytes = serde_json::to_vec_pretty(catalog)?;
    if bytes.len() as u64 > MAX_CATALOG_BYTES {
        return Err(LabError::InputTooLarge);
    }
    crate::store::write_file_atomic(store.storage_root(), &store.catalog_path_internal(), &bytes)
}

#[cfg(test)]
mod tests {
    use crate::{LabError, PrivateLabStore, SpecialistDraftRequestV1, SpecialistRunKindV1};

    fn request() -> SpecialistDraftRequestV1 {
        SpecialistDraftRequestV1 {
            schema_version: 1,
            specialist_id: "surrogate-experiment-reviewer".parse().unwrap(),
            name: "Surrogate Experiment Reviewer".to_owned(),
            purpose: "Review experiment evidence without overclaiming.".to_owned(),
        }
    }

    #[test]
    fn draft_and_public_synthetic_smoke_persist_without_claiming_training() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = PrivateLabStore::open_at(temp.path()).unwrap();
        let detail = store.create_specialist_draft(request()).unwrap();

        assert_eq!(detail.tests.len(), 1);
        assert_eq!(detail.tests[0].case_count, Some(12));
        assert!(!detail.tests[0].sealed);
        assert_eq!(detail.datasets.len(), 0);
        assert_eq!(detail.runs.len(), 1);
        assert_eq!(detail.runs[0].kind, SpecialistRunKindV1::SyntheticSmoke);
        assert_eq!(detail.runs[0].state, crate::JobState::Succeeded);
        assert!(detail.releases.is_empty());

        drop(store);
        let reopened = PrivateLabStore::open_at(temp.path()).unwrap();
        let listed = reopened.list_specialists().unwrap();
        assert_eq!(listed.len(), 1);
        let persisted = reopened
            .specialist_detail(&"surrogate-experiment-reviewer".parse().unwrap())
            .unwrap();
        assert_eq!(persisted, detail);
    }

    #[test]
    fn duplicate_or_invalid_draft_fails_without_rewriting_catalog() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = PrivateLabStore::open_at(temp.path()).unwrap();
        store.create_specialist_draft(request()).unwrap();
        let before = std::fs::read(store.catalog_path()).unwrap();

        assert!(matches!(
            store.create_specialist_draft(request()),
            Err(LabError::AlreadyExists)
        ));
        assert_eq!(std::fs::read(store.catalog_path()).unwrap(), before);

        let mut invalid = request();
        invalid.name = "   ".to_owned();
        assert!(matches!(
            store.create_specialist_draft(invalid),
            Err(LabError::InvalidContract(_))
        ));
        assert_eq!(std::fs::read(store.catalog_path()).unwrap(), before);
    }
}
