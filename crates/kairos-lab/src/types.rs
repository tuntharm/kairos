use crate::{DatasetState, ExperimentContractCheckV1, JobState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{collections::BTreeMap, fmt, str::FromStr};

use crate::LabError;

pub const V0_TRAIN_CASES: u32 = 72;
pub const V0_VALIDATION_CASES: u32 = 18;
pub const V0_TEST_CASES: u32 = 30;
pub const MAX_TRAINING_CONFIGS_V0: usize = 3;

/// A path-safe identifier used for every externally supplied registry key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableId(String);

impl StableId {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, LabError> {
        let value = value.as_ref();
        let valid = !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && value
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric);
        if !valid {
            return Err(LabError::InvalidStableId);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StableId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for StableId {
    type Err = LabError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for StableId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for StableId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

/// A canonical lowercase hexadecimal SHA-256 digest.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, LabError> {
        let value = value.as_ref();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(LabError::InvalidSha256);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for Sha256Digest {
    type Err = LabError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionStatusV1 {
    Accepted,
    WorkingHypothesis,
    Rejected,
    Deferred,
    NeedsSupervision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingV1 {
    pub claim: String,
    pub evidence_ids: Vec<StableId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceCitationV1 {
    pub evidence_id: StableId,
    pub claim: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NextExperimentV1 {
    pub changed_variable: String,
    pub fixed_controls: Vec<String>,
    pub metric: String,
    pub decision_rule: String,
    pub stop_condition: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperimentReviewV1 {
    pub assumptions: Vec<String>,
    pub decision_status: DecisionStatusV1,
    pub supported_findings: Vec<FindingV1>,
    pub unsupported_claims: Vec<FindingV1>,
    pub missing_evidence: Vec<String>,
    pub checker_result: ExperimentContractCheckV1,
    pub next_experiment: NextExperimentV1,
    pub citations: Vec<EvidenceCitationV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseOriginV1 {
    Synthetic,
    ConsentedExcerpt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatasetSplitV1 {
    Train,
    Validation,
    Test,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistCaseInputV1 {
    pub contract_version: u16,
    pub question: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextEvidenceV1 {
    pub evidence_id: StableId,
    pub content_sha256: Sha256Digest,
    pub excerpt: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CaseProvenanceV1 {
    pub source_manifest_sha256: Sha256Digest,
    pub origin: CaseOriginV1,
    pub reviewer: StableId,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetCaseV1 {
    pub schema_version: u16,
    pub case_id: StableId,
    pub scenario_group: StableId,
    pub origin: CaseOriginV1,
    pub source_evidence_ids: Vec<StableId>,
    pub input: SpecialistCaseInputV1,
    pub context: Vec<ContextEvidenceV1>,
    pub provenance: CaseProvenanceV1,
    pub gold_review: ExperimentReviewV1,
    pub gold_citations: Vec<EvidenceCitationV1>,
    pub expected_checker_findings: ExperimentContractCheckV1,
    pub reviewer: StableId,
    pub split: DatasetSplitV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationSettingsV1 {
    pub temperature: f64,
    pub top_p: f64,
    pub max_tokens: u32,
    pub seed: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelIdentityV1 {
    pub repository: String,
    pub immutable_revision: String,
    pub quantisation: String,
    pub local_file_sha256: BTreeMap<String, Sha256Digest>,
    pub licence: String,
    pub tokenizer_sha256: Sha256Digest,
    pub chat_template_sha256: Sha256Digest,
    pub runtime_lock_sha256: Sha256Digest,
    pub generation: GenerationSettingsV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrainingConfigV1 {
    pub schema_version: u16,
    pub config_id: StableId,
    pub seed: u64,
    pub lora_rank: u16,
    pub lora_layers: u16,
    pub batch_size: u16,
    pub gradient_accumulation_steps: u16,
    pub learning_rate: f64,
    pub max_sequence_length: u32,
    pub smoke_iterations: u32,
    pub training_iterations: u32,
}

impl TrainingConfigV1 {
    pub fn educational_v0() -> Self {
        Self {
            schema_version: 1,
            config_id: StableId::parse("educational-qlora-v0")
                .expect("the built-in config ID is valid"),
            seed: 42,
            lora_rank: 8,
            lora_layers: 8,
            batch_size: 1,
            gradient_accumulation_steps: 4,
            learning_rate: 1e-5,
            max_sequence_length: 2_048,
            smoke_iterations: 20,
            training_iterations: 300,
        }
    }

    pub fn is_exact_educational_v0(&self) -> bool {
        self == &Self::educational_v0()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerRuntimeIdentityV1 {
    pub python_version: String,
    pub environment_lock_sha256: Sha256Digest,
    pub worker_sha256: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecialistSpecV1 {
    pub schema_version: u16,
    pub specialist_id: StableId,
    pub purpose: String,
    pub output_schema_version: u16,
    pub instructions_sha256: Sha256Digest,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceManifestV1 {
    pub schema_version: u16,
    pub source_id: StableId,
    pub specialist_id: StableId,
    pub content_sha256: Sha256Digest,
    pub evidence_ids: Vec<StableId>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatasetManifestV1 {
    pub schema_version: u16,
    pub dataset_id: StableId,
    pub specialist_id: StableId,
    pub state: DatasetState,
    pub source_manifest_sha256: Sha256Digest,
    pub cases_sha256: Sha256Digest,
    pub split_counts: BTreeMap<DatasetSplitV1, u32>,
    pub scenario_group_splits: BTreeMap<StableId, DatasetSplitV1>,
    pub created_at: DateTime<Utc>,
    pub frozen_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunManifestV1 {
    pub schema_version: u16,
    pub run_id: StableId,
    pub specialist_id: StableId,
    pub dataset_sha256: Sha256Digest,
    pub state: JobState,
    pub model: ModelIdentityV1,
    pub worker_runtime: WorkerRuntimeIdentityV1,
    pub training_config: TrainingConfigV1,
    pub training_config_sha256: Sha256Digest,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessVerdictV1 {
    Eligible,
    NotEligible,
    Inconclusive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseManifestV1 {
    pub schema_version: u16,
    pub specialist_id: StableId,
    pub release_id: StableId,
    pub base_sha256: Sha256Digest,
    pub adapter_sha256: Option<Sha256Digest>,
    pub instructions_sha256: Sha256Digest,
    pub source_sha256: Sha256Digest,
    pub dataset_sha256: Sha256Digest,
    pub tool_sha256: Sha256Digest,
    pub evaluation_id: StableId,
    pub evaluation_sha256: Sha256Digest,
    pub readiness_verdict: ReadinessVerdictV1,
    pub failure_reason: Option<String>,
    pub active_release_id: Option<StableId>,
    pub previous_release_id: Option<StableId>,
    pub created_at: DateTime<Utc>,
    pub activated_at: Option<DateTime<Utc>>,
    pub evaluated_boundary_sha256: Sha256Digest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_and_hashes_fail_closed() {
        assert!(StableId::parse("surrogate-experiment-reviewer").is_ok());
        for unsafe_id in ["", "../escape", "/absolute", "two words", "UPPER"] {
            assert!(
                StableId::parse(unsafe_id).is_err(),
                "accepted {unsafe_id:?}"
            );
        }

        assert!(Sha256Digest::parse("a".repeat(64)).is_ok());
        assert!(Sha256Digest::parse("A".repeat(64)).is_err());
        assert!(Sha256Digest::parse("abc").is_err());
    }

    #[test]
    fn educational_training_config_is_the_frozen_bounded_default() {
        let config = TrainingConfigV1::educational_v0();
        assert!(config.is_exact_educational_v0());
        assert_eq!(config.seed, 42);
        assert_eq!(config.smoke_iterations, 20);
        assert_eq!(config.training_iterations, 300);
        assert_eq!(MAX_TRAINING_CONFIGS_V0, 3);
        assert_eq!(V0_TRAIN_CASES + V0_VALIDATION_CASES + V0_TEST_CASES, 120);
    }
}
