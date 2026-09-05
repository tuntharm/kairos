use kairos_lab::{
    ExperimentReviewV1, LabError, PrivateLabStore, ReleaseManifestV1, SpecialistComparisonV1,
    SpecialistDetailV1, SpecialistDraftRequestV1, SpecialistRunSummaryV1, SpecialistSummaryV1,
    StableId,
};
use serde::{Deserialize, Serialize};

const MAX_SPECIALIST_INPUT_CHARS: usize = 32_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LabCommandErrorKind {
    Unavailable,
    InvalidRequest,
    NotFound,
    Conflict,
    ManualApprovalRequired,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LabCommandError {
    kind: LabCommandErrorKind,
    message: String,
}

impl LabCommandError {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: LabCommandErrorKind::Unavailable,
            message: message.into(),
        }
    }

    fn manual_approval(message: impl Into<String>) -> Self {
        Self {
            kind: LabCommandErrorKind::ManualApprovalRequired,
            message: message.into(),
        }
    }
}

impl From<LabError> for LabCommandError {
    fn from(error: LabError) -> Self {
        let (kind, message) = match error {
            LabError::InvalidStableId
            | LabError::InvalidSha256
            | LabError::InvalidTransition
            | LabError::UnsupportedVersion
            | LabError::InvalidContract(_)
            | LabError::InputTooLarge => (
                LabCommandErrorKind::InvalidRequest,
                "The Specialist Lab request failed its native contract checks.".to_owned(),
            ),
            LabError::NotFound => (
                LabCommandErrorKind::NotFound,
                "The requested Specialist Lab record does not exist.".to_owned(),
            ),
            LabError::AlreadyExists | LabError::RegistryLocked | LabError::StaleRegistry => (
                LabCommandErrorKind::Conflict,
                "Specialist Lab state changed or is currently being updated. Refresh and try again."
                    .to_owned(),
            ),
            LabError::NotEligible => (
                LabCommandErrorKind::Conflict,
                "That release is not eligible for activation.".to_owned(),
            ),
            LabError::UnsafeProcessPath | LabError::RuntimeIdentityMismatch => (
                LabCommandErrorKind::Unavailable,
                "The managed local training runtime is missing or failed its integrity check."
                    .to_owned(),
            ),
            LabError::WorkerCancelled => (
                LabCommandErrorKind::Conflict,
                "The local Specialist Lab job was cancelled.".to_owned(),
            ),
            LabError::WorkerTimedOut => (
                LabCommandErrorKind::Failed,
                "The local Specialist Lab job exceeded its fixed timeout.".to_owned(),
            ),
            LabError::WorkerProtocol(_) => (
                LabCommandErrorKind::Failed,
                "The managed local worker returned an invalid result.".to_owned(),
            ),
            LabError::Io(_) | LabError::Json(_) => (
                LabCommandErrorKind::Failed,
                "Kairos could not read or persist private Specialist Lab state.".to_owned(),
            ),
        };
        Self { kind, message }
    }
}

fn open_store() -> Result<PrivateLabStore, LabCommandError> {
    PrivateLabStore::open_default().map_err(Into::into)
}

fn parse_id(value: &str) -> Result<StableId, LabCommandError> {
    StableId::parse(value).map_err(Into::into)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SpecialistExecutionRequestV1 {
    schema_version: u16,
    specialist_id: String,
    release_id: String,
    input: String,
    session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpecialistResponseIdentityV1 {
    schema_version: u16,
    specialist_id: StableId,
    specialist_name: String,
    release_id: StableId,
    release_sha256: String,
    evaluation_sha256: String,
    evidence_boundary_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpecialistExecutionV1 {
    schema_version: u16,
    output: ExperimentReviewV1,
    identity: SpecialistResponseIdentityV1,
}

#[tauri::command]
pub(crate) fn list_specialists() -> Result<Vec<SpecialistSummaryV1>, LabCommandError> {
    open_store()?.list_specialists().map_err(Into::into)
}

#[tauri::command]
pub(crate) fn get_specialist(specialist_id: String) -> Result<SpecialistDetailV1, LabCommandError> {
    open_store()?
        .specialist_detail(&parse_id(&specialist_id)?)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn create_specialist_draft(
    request: SpecialistDraftRequestV1,
) -> Result<SpecialistDetailV1, LabCommandError> {
    open_store()?
        .create_specialist_draft(request)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn freeze_specialist_dataset(
    specialist_id: String,
    dataset_id: String,
) -> Result<SpecialistDetailV1, LabCommandError> {
    let store = open_store()?;
    let specialist_id = parse_id(&specialist_id)?;
    let _dataset_id = parse_id(&dataset_id)?;
    store.specialist_detail(&specialist_id)?;
    Err(LabCommandError::unavailable(
        "No reviewed 72/18/30 dataset is registered yet. Kairos did not freeze or expose any source material.",
    ))
}

#[tauri::command]
pub(crate) fn run_specialist_baseline(
    specialist_id: String,
    dataset_id: String,
) -> Result<SpecialistRunSummaryV1, LabCommandError> {
    let store = open_store()?;
    store.specialist_detail(&parse_id(&specialist_id)?)?;
    let _dataset_id = parse_id(&dataset_id)?;
    Err(LabCommandError::manual_approval(
        "The untouched-model baseline is paused until approved sources, the frozen dataset, and the pinned model/runtime preflight are complete. No model was run.",
    ))
}

#[tauri::command]
pub(crate) fn start_specialist_training(
    specialist_id: String,
    dataset_id: String,
) -> Result<SpecialistRunSummaryV1, LabCommandError> {
    let store = open_store()?;
    store.specialist_detail(&parse_id(&specialist_id)?)?;
    let _dataset_id = parse_id(&dataset_id)?;
    Err(LabCommandError::manual_approval(
        "Training requires a separate approval after Kairos shows the exact model revision, destination, download size, free disk, memory estimate, and measured smoke-run time. No job was created.",
    ))
}

#[tauri::command]
pub(crate) fn get_specialist_run(
    specialist_id: String,
    run_id: String,
) -> Result<SpecialistRunSummaryV1, LabCommandError> {
    open_store()?
        .specialist_run(&parse_id(&specialist_id)?, &parse_id(&run_id)?)
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn cancel_specialist_run(
    specialist_id: String,
    run_id: String,
) -> Result<SpecialistRunSummaryV1, LabCommandError> {
    let store = open_store()?;
    let run = store.specialist_run(&parse_id(&specialist_id)?, &parse_id(&run_id)?)?;
    if matches!(
        run.state,
        kairos_lab::JobState::Succeeded
            | kairos_lab::JobState::Failed
            | kairos_lab::JobState::Cancelled
            | kairos_lab::JobState::Interrupted
    ) {
        return Err(LabCommandError {
            kind: LabCommandErrorKind::Conflict,
            message: "That Specialist Lab run is already terminal and was not changed.".to_owned(),
        });
    }
    Err(LabCommandError::unavailable(
        "No managed asynchronous Specialist Lab runner is active in this build.",
    ))
}

#[tauri::command]
pub(crate) fn evaluate_specialist_candidate(
    specialist_id: String,
    candidate_id: String,
) -> Result<SpecialistComparisonV1, LabCommandError> {
    open_store()?.specialist_detail(&parse_id(&specialist_id)?)?;
    let _candidate_id = parse_id(&candidate_id)?;
    Err(LabCommandError::manual_approval(
        "Opening the sealed held-out test is a separate approval gate. Kairos did not evaluate or promote this candidate.",
    ))
}

#[tauri::command]
pub(crate) fn get_specialist_release(
    specialist_id: String,
    release_id: String,
) -> Result<ReleaseManifestV1, LabCommandError> {
    let specialist_id = parse_id(&specialist_id)?;
    let release_id = parse_id(&release_id)?;
    open_store()?
        .release_state(&specialist_id)?
        .releases
        .remove(&release_id)
        .ok_or_else(|| LabError::NotFound.into())
}

#[tauri::command]
pub(crate) fn activate_specialist_release(
    specialist_id: String,
    release_id: String,
    expected_active_release_id: Option<String>,
) -> Result<ReleaseManifestV1, LabCommandError> {
    let specialist_id = parse_id(&specialist_id)?;
    let release_id = parse_id(&release_id)?;
    let expected = expected_active_release_id
        .as_deref()
        .map(parse_id)
        .transpose()?;
    let mut store = open_store()?;
    store.activate(&specialist_id, &release_id, expected.as_ref())?;
    store
        .release_state(&specialist_id)?
        .releases
        .remove(&release_id)
        .ok_or_else(|| LabError::NotFound.into())
}

#[tauri::command]
pub(crate) fn rollback_specialist_release(
    specialist_id: String,
    expected_active_release_id: String,
) -> Result<ReleaseManifestV1, LabCommandError> {
    let specialist_id = parse_id(&specialist_id)?;
    let expected = parse_id(&expected_active_release_id)?;
    let mut store = open_store()?;
    store.rollback(&specialist_id, Some(&expected))?;
    let mut state = store.release_state(&specialist_id)?;
    let active = state.active_release_id.ok_or(LabError::NotFound)?;
    state
        .releases
        .remove(&active)
        .ok_or_else(|| LabError::NotFound.into())
}

#[tauri::command]
pub(crate) fn run_specialist(
    request: SpecialistExecutionRequestV1,
) -> Result<SpecialistExecutionV1, LabCommandError> {
    if request.schema_version != 1
        || request.input.trim().is_empty()
        || request.input.chars().count() > MAX_SPECIALIST_INPUT_CHARS
        || request
            .session_id
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 160)
    {
        return Err(LabCommandError {
            kind: LabCommandErrorKind::InvalidRequest,
            message: "Specialist input failed the native size or schema checks.".to_owned(),
        });
    }
    let specialist_id = parse_id(&request.specialist_id)?;
    let expected_release_id = parse_id(&request.release_id)?;
    let store = open_store()?;
    store.specialist_detail(&specialist_id)?;
    let state = store.release_state(&specialist_id)?;
    let active_release_id = state.active_release_id.ok_or_else(|| {
        LabCommandError::unavailable(
            "This specialist has no human-activated eligible release. Kairos did not fall back to a manager model.",
        )
    })?;
    if active_release_id != expected_release_id {
        return Err(LabCommandError {
            kind: LabCommandErrorKind::Conflict,
            message: "The active specialist release changed before execution. Review the new release and submit again; Kairos did not run or fall back.".to_owned(),
        });
    }
    Err(LabCommandError::unavailable(
        "The direct MLX specialist execution runtime has not been approved and installed. Kairos did not fall back or send this input elsewhere.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_typed_and_do_not_expose_io_details() {
        let error = LabCommandError::from(LabError::Io(std::io::Error::other(
            "/private/path should stay hidden",
        )));
        assert_eq!(error.kind, LabCommandErrorKind::Failed);
        assert!(!error.message.contains("/private/path"));

        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json["kind"], "failed");
        assert!(
            json["message"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
    }

    #[test]
    fn specialist_execution_request_rejects_unknown_fields() {
        let input = serde_json::json!({
            "schemaVersion": 1,
            "specialistId": "surrogate-experiment-reviewer",
            "releaseId": "release-001",
            "input": "Review this experiment",
            "sessionId": null,
            "arbitraryPath": "/tmp/private"
        });
        assert!(serde_json::from_value::<SpecialistExecutionRequestV1>(input).is_err());
    }

    #[test]
    fn specialist_execution_request_requires_the_displayed_release_id() {
        let missing_release = serde_json::json!({
            "schemaVersion": 1,
            "specialistId": "surrogate-experiment-reviewer",
            "input": "Review this experiment",
            "sessionId": null
        });
        assert!(
            serde_json::from_value::<SpecialistExecutionRequestV1>(missing_release).is_err(),
            "execution must pin the exact active release shown at submit time"
        );
    }
}
