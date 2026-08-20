use crate::{
    DatasetCaseV1, DatasetManifestV1, DatasetSplitV1, DatasetState, LabError, Sha256Digest,
    StableId, V0_TEST_CASES, V0_TRAIN_CASES, V0_VALIDATION_CASES,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingDatasetViewV1 {
    pub schema_version: u16,
    pub dataset_id: StableId,
    pub dataset_sha256: Sha256Digest,
    pub train_case_ids: Vec<StableId>,
    pub validation_case_ids: Vec<StableId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatasetStorageLayout {
    worker_train_validation_root: PathBuf,
    sealed_test_root: PathBuf,
}

impl DatasetStorageLayout {
    pub(crate) fn create(lab_root: &Path, dataset_id: &StableId) -> Result<Self, LabError> {
        let dataset_root = private_child(lab_root, "datasets")?;
        let dataset_root = private_child(&dataset_root, dataset_id.as_str())?;
        let worker_root = private_child(&dataset_root, "worker")?;
        let worker_train_validation_root = private_child(&worker_root, "train-validation")?;
        let sealed_test_root = private_child(&dataset_root, "sealed-test")?;
        let worker_train_validation_root = fs::canonicalize(worker_train_validation_root)?;
        let sealed_test_root = fs::canonicalize(sealed_test_root)?;
        if sealed_test_root.starts_with(&worker_train_validation_root) {
            return Err(LabError::InvalidContract(
                "sealed test is inside worker data",
            ));
        }
        Ok(Self {
            worker_train_validation_root,
            sealed_test_root,
        })
    }

    pub fn worker_train_validation_root(&self) -> &Path {
        &self.worker_train_validation_root
    }

    pub fn sealed_test_root(&self) -> &Path {
        &self.sealed_test_root
    }
}

fn private_child(parent: &Path, component: &str) -> Result<PathBuf, LabError> {
    let path = parent.join(component);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(LabError::InvalidContract(
                "dataset storage component is unsafe",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&path)?,
        Err(error) => return Err(error.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(path)
}

pub fn validate_grouped_dataset(
    manifest: &DatasetManifestV1,
    cases: &[DatasetCaseV1],
) -> Result<(), LabError> {
    if manifest.schema_version != 1 {
        return Err(LabError::UnsupportedVersion);
    }
    let mut case_ids = BTreeSet::new();
    let mut counts = BTreeMap::<DatasetSplitV1, u32>::new();
    let mut group_splits = BTreeMap::<StableId, DatasetSplitV1>::new();

    for case in cases {
        if case.schema_version != 1
            || case.input.contract_version != 1
            || case.input.question.trim().is_empty()
            || case.context.is_empty()
            || case.source_evidence_ids.is_empty()
            || case.provenance.origin != case.origin
            || case.provenance.reviewer != case.reviewer
        {
            return Err(LabError::InvalidContract(
                "dataset case metadata is incomplete",
            ));
        }
        if !case_ids.insert(case.case_id.clone()) {
            return Err(LabError::InvalidContract("duplicate dataset case ID"));
        }
        if group_splits
            .insert(case.scenario_group.clone(), case.split)
            .is_some_and(|existing| existing != case.split)
        {
            return Err(LabError::InvalidContract("scenario group crosses splits"));
        }
        let source_ids = case.source_evidence_ids.iter().collect::<BTreeSet<_>>();
        if case
            .context
            .iter()
            .any(|item| !source_ids.contains(&item.evidence_id) || item.excerpt.trim().is_empty())
        {
            return Err(LabError::InvalidContract(
                "context provenance is inconsistent",
            ));
        }
        *counts.entry(case.split).or_default() += 1;
    }

    if counts != manifest.split_counts || group_splits != manifest.scenario_group_splits {
        return Err(LabError::InvalidContract(
            "dataset manifest does not match cases",
        ));
    }
    Ok(())
}

pub fn validate_v0_split_shape(manifest: &DatasetManifestV1) -> Result<(), LabError> {
    let expected = BTreeMap::from([
        (DatasetSplitV1::Train, V0_TRAIN_CASES),
        (DatasetSplitV1::Validation, V0_VALIDATION_CASES),
        (DatasetSplitV1::Test, V0_TEST_CASES),
    ]);
    if manifest.split_counts != expected {
        return Err(LabError::InvalidContract("V0 split must be 72/18/30"));
    }
    Ok(())
}

/// Projects a frozen dataset into the only worker-visible view; sealed test IDs are absent.
pub fn training_dataset_view(
    manifest: &DatasetManifestV1,
    cases: &[DatasetCaseV1],
) -> Result<TrainingDatasetViewV1, LabError> {
    if manifest.state != DatasetState::Frozen {
        return Err(LabError::InvalidContract("training dataset is not frozen"));
    }
    validate_grouped_dataset(manifest, cases)?;
    let mut train_case_ids = Vec::new();
    let mut validation_case_ids = Vec::new();
    for case in cases {
        match case.split {
            DatasetSplitV1::Train => train_case_ids.push(case.case_id.clone()),
            DatasetSplitV1::Validation => validation_case_ids.push(case.case_id.clone()),
            DatasetSplitV1::Test => {}
        }
    }
    train_case_ids.sort();
    validation_case_ids.sort();
    Ok(TrainingDatasetViewV1 {
        schema_version: 1,
        dataset_id: manifest.dataset_id.clone(),
        dataset_sha256: manifest.cases_sha256.clone(),
        train_case_ids,
        validation_case_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ContextEvidenceV1, synthetic_fixture_cases};
    use chrono::Utc;

    fn digest(character: char) -> Sha256Digest {
        Sha256Digest::parse(character.to_string().repeat(64)).unwrap()
    }

    fn v0_dataset() -> (DatasetManifestV1, Vec<DatasetCaseV1>) {
        let template = synthetic_fixture_cases().unwrap().remove(0);
        let mut cases = Vec::new();
        for (split, count, prefix) in [
            (DatasetSplitV1::Train, V0_TRAIN_CASES, "train"),
            (
                DatasetSplitV1::Validation,
                V0_VALIDATION_CASES,
                "validation",
            ),
            (DatasetSplitV1::Test, V0_TEST_CASES, "test"),
        ] {
            for index in 0..count {
                let mut case = template.clone();
                case.case_id = StableId::parse(format!("{prefix}-case-{index:03}")).unwrap();
                case.scenario_group =
                    StableId::parse(format!("{prefix}-group-{:03}", index / 3)).unwrap();
                case.split = split;
                let evidence_id =
                    StableId::parse(format!("synthetic-{prefix}-evidence-{index:03}")).unwrap();
                case.source_evidence_ids = vec![evidence_id.clone()];
                case.context = vec![ContextEvidenceV1 {
                    evidence_id: evidence_id.clone(),
                    content_sha256: digest('a'),
                    excerpt: "Public synthetic experiment evidence.".to_owned(),
                }];
                case.gold_review.supported_findings[0].evidence_ids = vec![evidence_id.clone()];
                case.gold_review.citations[0].evidence_id = evidence_id.clone();
                case.gold_citations[0].evidence_id = evidence_id;
                cases.push(case);
            }
        }
        let split_counts = BTreeMap::from([
            (DatasetSplitV1::Train, V0_TRAIN_CASES),
            (DatasetSplitV1::Validation, V0_VALIDATION_CASES),
            (DatasetSplitV1::Test, V0_TEST_CASES),
        ]);
        let scenario_group_splits = cases
            .iter()
            .map(|case| (case.scenario_group.clone(), case.split))
            .collect();
        let manifest = DatasetManifestV1 {
            schema_version: 1,
            dataset_id: StableId::parse("reviewer-dataset-v0").unwrap(),
            specialist_id: StableId::parse("surrogate-experiment-reviewer").unwrap(),
            state: DatasetState::Frozen,
            source_manifest_sha256: digest('b'),
            cases_sha256: digest('c'),
            split_counts,
            scenario_group_splits,
            created_at: Utc::now(),
            frozen_at: Some(Utc::now()),
        };
        (manifest, cases)
    }

    #[test]
    fn v0_shape_is_exact_and_training_projection_cannot_contain_sealed_test_ids() {
        let (manifest, cases) = v0_dataset();
        validate_v0_split_shape(&manifest).unwrap();
        validate_grouped_dataset(&manifest, &cases).unwrap();
        let view = training_dataset_view(&manifest, &cases).unwrap();
        assert_eq!(view.train_case_ids.len(), 72);
        assert_eq!(view.validation_case_ids.len(), 18);
        assert!(
            view.train_case_ids
                .iter()
                .chain(&view.validation_case_ids)
                .all(|id| !id.as_str().starts_with("test-"))
        );
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("testCase"));
        assert!(!json.contains("testPath"));
    }

    #[test]
    fn scenario_group_cross_split_is_rejected() {
        let (mut manifest, mut cases) = v0_dataset();
        let test_group = cases
            .iter()
            .find(|case| case.split == DatasetSplitV1::Test)
            .unwrap()
            .scenario_group
            .clone();
        let train = cases
            .iter_mut()
            .find(|case| case.split == DatasetSplitV1::Train)
            .unwrap();
        let old_group = train.scenario_group.clone();
        train.scenario_group = test_group.clone();
        manifest.scenario_group_splits.remove(&old_group);
        manifest
            .scenario_group_splits
            .insert(test_group, DatasetSplitV1::Test);
        assert!(validate_grouped_dataset(&manifest, &cases).is_err());
    }

    #[test]
    fn shared_approved_source_identity_does_not_define_scenario_leakage() {
        let (manifest, mut cases) = v0_dataset();
        let shared_id = cases
            .iter()
            .find(|case| case.split == DatasetSplitV1::Train)
            .unwrap()
            .source_evidence_ids[0]
            .clone();
        let validation = cases
            .iter_mut()
            .find(|case| case.split == DatasetSplitV1::Validation)
            .unwrap();
        validation.source_evidence_ids = vec![shared_id.clone()];
        validation.context[0].evidence_id = shared_id.clone();
        validation.gold_review.supported_findings[0].evidence_ids = vec![shared_id.clone()];
        validation.gold_review.citations[0].evidence_id = shared_id.clone();
        validation.gold_citations[0].evidence_id = shared_id;
        validate_grouped_dataset(&manifest, &cases).unwrap();
    }

    #[test]
    fn sealed_test_storage_is_a_physical_sibling_not_worker_input() {
        let temp = tempfile::tempdir().unwrap();
        let layout = DatasetStorageLayout::create(
            temp.path(),
            &StableId::parse("reviewer-dataset-v0").unwrap(),
        )
        .unwrap();
        assert!(layout.worker_train_validation_root().is_dir());
        assert!(layout.sealed_test_root().is_dir());
        let sealed_file = layout.sealed_test_root().join("cases.jsonl");
        fs::write(&sealed_file, b"synthetic sealed marker\n").unwrap();
        let sealed_file = fs::canonicalize(sealed_file).unwrap();
        assert!(
            !layout
                .sealed_test_root()
                .starts_with(layout.worker_train_validation_root())
        );
        assert!(!sealed_file.starts_with(layout.worker_train_validation_root()));
    }
}
