use crate::{
    DatasetStorageLayout, EvaluationReportV1, JobEventV1, JobState, LabError, ReadinessVerdictV1,
    ReleaseManifestV1, Sha256Digest, StableId, assess_candidate_eligibility,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const REGISTRY_SCHEMA_VERSION: u16 = 1;
const MAX_EVENT_BYTES: usize = 16 * 1024;
const MAX_REGISTRY_BYTES: u64 = 4 * 1024 * 1024;
const LOCK_STALE_AFTER: Duration = Duration::from_secs(30);
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReleaseStateV1 {
    pub active_release_id: Option<StableId>,
    pub previous_release_id: Option<StableId>,
    pub releases: BTreeMap<StableId, ReleaseManifestV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryV1 {
    schema_version: u16,
    specialists: BTreeMap<StableId, ReleaseStateV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobTerminalRecordV1 {
    pub schema_version: u16,
    pub run_id: StableId,
    pub state: JobState,
    pub reason_code: StableId,
    pub recorded_at: chrono::DateTime<Utc>,
}

impl Default for RegistryV1 {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            specialists: BTreeMap::new(),
        }
    }
}

pub struct PrivateLabStore {
    root: PathBuf,
    registry: RegistryV1,
}

impl PrivateLabStore {
    /// Opens the only production storage root. Callers cannot supply a path.
    pub fn open_default() -> Result<Self, LabError> {
        let home =
            std::env::var_os("HOME").ok_or(LabError::InvalidContract("HOME is unavailable"))?;
        let home = fs::canonicalize(Path::new(&home))?;
        let application_support = home.join("Library/Application Support");
        reject_existing_symlink_descendants(&home, &application_support)?;
        Self::open_application_support(application_support)
    }

    fn open_application_support(application_support: PathBuf) -> Result<Self, LabError> {
        fs::create_dir_all(&application_support)?;
        let application_support = fs::canonicalize(&application_support)?;
        if !fs::metadata(&application_support)?.is_dir() {
            return Err(LabError::InvalidContract(
                "application support root is not a directory",
            ));
        }
        let root = ensure_private_child_tree(&application_support, &["Kairos", "lab", "v1"])?;
        let registry_path = root.join("registry.json");
        let registry = if registry_path.exists() {
            let parsed = read_registry(&registry_path)?;
            if parsed.schema_version != REGISTRY_SCHEMA_VERSION {
                return Err(LabError::UnsupportedVersion);
            }
            parsed
        } else {
            RegistryV1::default()
        };
        Ok(Self { root, registry })
    }

    #[cfg(test)]
    pub(crate) fn open_at(application_support: &Path) -> Result<Self, LabError> {
        Self::open_application_support(application_support.to_path_buf())
    }

    #[cfg(test)]
    fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn storage_root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn catalog_path_internal(&self) -> PathBuf {
        // Keep the filename fixed; callers never provide storage paths.
        self.root.join("catalog.json")
    }

    pub fn prepare_dataset_storage(
        &self,
        dataset_id: &StableId,
    ) -> Result<DatasetStorageLayout, LabError> {
        DatasetStorageLayout::create(&self.root, dataset_id)
    }

    pub fn register_release(
        &mut self,
        mut release: ReleaseManifestV1,
        evaluation: &EvaluationReportV1,
    ) -> Result<(), LabError> {
        if evaluation.specialist_id != release.specialist_id {
            return Err(LabError::InvalidContract("evaluation specialist mismatch"));
        }
        let decision = assess_candidate_eligibility(evaluation);
        release.evaluation_id = evaluation.evaluation_id.clone();
        release.evaluation_sha256 = evaluation_digest(evaluation)?;
        release.evaluated_boundary_sha256 = evaluation.evaluated_boundary_sha256.clone();
        release.readiness_verdict = decision.verdict;
        release.failure_reason = if decision.verdict == ReadinessVerdictV1::Eligible {
            None
        } else {
            Some(decision.reasons.join("; "))
        };
        validate_release(&release)?;
        let root = self.root.clone();
        self.update_registry(move |registry| {
            persist_evaluation(&root, evaluation)?;
            let state = registry
                .specialists
                .entry(release.specialist_id.clone())
                .or_default();
            if state.releases.contains_key(&release.release_id) {
                return Err(LabError::AlreadyExists);
            }
            state.releases.insert(release.release_id.clone(), release);
            Ok(())
        })
    }

    pub fn activate(
        &mut self,
        specialist_id: &StableId,
        release_id: &StableId,
        expected_active_release_id: Option<&StableId>,
    ) -> Result<(), LabError> {
        let root = self.root.clone();
        self.update_registry(move |registry| {
            let state = registry
                .specialists
                .get_mut(specialist_id)
                .ok_or(LabError::NotFound)?;
            if state.active_release_id.as_ref() != expected_active_release_id {
                return Err(LabError::StaleRegistry);
            }
            if state.active_release_id.as_ref() == Some(release_id) {
                return Err(LabError::StaleRegistry);
            }
            let release = state.releases.get(release_id).ok_or(LabError::NotFound)?;
            validate_persisted_evaluation(&root, release)?;
            if release.specialist_id != *specialist_id
                || release.readiness_verdict != ReadinessVerdictV1::Eligible
                || release.failure_reason.is_some()
            {
                return Err(LabError::NotEligible);
            }

            let previous = state.active_release_id.clone();
            state.previous_release_id = previous;
            state.active_release_id = Some(release_id.clone());
            project_active_release(state, Utc::now());
            Ok(())
        })
    }

    pub fn rollback(
        &mut self,
        specialist_id: &StableId,
        expected_active_release_id: Option<&StableId>,
    ) -> Result<(), LabError> {
        let root = self.root.clone();
        self.update_registry(move |registry| {
            let state = registry
                .specialists
                .get_mut(specialist_id)
                .ok_or(LabError::NotFound)?;
            if state.active_release_id.as_ref() != expected_active_release_id {
                return Err(LabError::StaleRegistry);
            }
            let current = state.active_release_id.clone().ok_or(LabError::NotFound)?;
            let previous = state
                .previous_release_id
                .clone()
                .ok_or(LabError::NotFound)?;
            let release = state.releases.get(&previous).ok_or(LabError::NotFound)?;
            validate_persisted_evaluation(&root, release)?;
            if release.readiness_verdict != ReadinessVerdictV1::Eligible
                || release.failure_reason.is_some()
            {
                return Err(LabError::NotEligible);
            }

            state.active_release_id = Some(previous.clone());
            state.previous_release_id = Some(current);
            project_active_release(state, Utc::now());
            Ok(())
        })
    }

    pub fn release_state(&self, specialist_id: &StableId) -> Result<ReleaseStateV1, LabError> {
        let registry_path = self.root.join("registry.json");
        let registry = if registry_path.exists() {
            read_registry(&registry_path)?
        } else {
            self.registry.clone()
        };
        registry
            .specialists
            .get(specialist_id)
            .cloned()
            .ok_or(LabError::NotFound)
    }

    /// Appends one typed, validated and redacted event under a derived run path.
    pub fn append_job_event(&self, event: &JobEventV1) -> Result<(), LabError> {
        event.validate()?;
        let event_line = serde_json::to_vec(event)?;
        if event_line.len() > MAX_EVENT_BYTES {
            return Err(LabError::InputTooLarge);
        }
        let mut lock = RegistryLock::acquire(&self.root)?;
        lock.heartbeat()?;
        let run_root = ensure_private_child_tree(&self.root, &["jobs", event.run_id.as_str()])?;
        let event_path = run_root.join("events.ndjson");
        reject_symlink_if_present(&event_path)?;
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        set_private_file_mode(&mut options);
        let mut file = options.open(event_path)?;
        file.write_all(&event_line)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        Ok(())
    }

    pub fn persist_job_terminal(&self, record: &JobTerminalRecordV1) -> Result<(), LabError> {
        if record.schema_version != 1
            || !matches!(
                record.state,
                JobState::Succeeded
                    | JobState::Failed
                    | JobState::Cancelled
                    | JobState::Interrupted
            )
        {
            return Err(LabError::InvalidContract("job record is not terminal"));
        }
        let mut lock = RegistryLock::acquire(&self.root)?;
        lock.heartbeat()?;
        let directory = ensure_private_child_tree(&self.root, &["jobs", record.run_id.as_str()])?;
        let path = directory.join("terminal.json");
        let bytes = serde_json::to_vec_pretty(record)?;
        if path.exists() {
            if fs::read(&path)? == bytes {
                return Ok(());
            }
            return Err(LabError::AlreadyExists);
        }
        write_file_atomic(&self.root, &path, &bytes)
    }

    pub fn load_job_terminal(&self, run_id: &StableId) -> Result<JobTerminalRecordV1, LabError> {
        let path = self
            .root
            .join("jobs")
            .join(run_id.as_str())
            .join("terminal.json");
        reject_symlink(&path)?;
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    }

    fn update_registry<F>(&mut self, update: F) -> Result<(), LabError>
    where
        F: FnOnce(&mut RegistryV1) -> Result<(), LabError>,
    {
        let mut lock = RegistryLock::acquire(&self.root)?;
        // Refresh while holding the lock so compare-and-swap sees another process's write.
        let registry_path = self.root.join("registry.json");
        let mut next = if registry_path.exists() {
            read_registry(&registry_path)?
        } else {
            self.registry.clone()
        };
        if next.schema_version != REGISTRY_SCHEMA_VERSION {
            return Err(LabError::UnsupportedVersion);
        }
        update(&mut next)?;
        lock.heartbeat()?;
        write_registry_atomic(&self.root, &next)?;
        self.registry = next;
        Ok(())
    }
}

fn evaluation_digest(evaluation: &EvaluationReportV1) -> Result<Sha256Digest, LabError> {
    let bytes = serde_json::to_vec_pretty(evaluation)?;
    Sha256Digest::parse(format!("{:x}", Sha256::digest(bytes)))
}

fn evaluation_path(root: &Path, evaluation_id: &StableId) -> Result<PathBuf, LabError> {
    let directory = ensure_private_child_tree(root, &["evaluations", evaluation_id.as_str()])?;
    Ok(directory.join("report.json"))
}

fn persist_evaluation(root: &Path, evaluation: &EvaluationReportV1) -> Result<(), LabError> {
    let path = evaluation_path(root, &evaluation.evaluation_id)?;
    let bytes = serde_json::to_vec_pretty(evaluation)?;
    if path.exists() {
        reject_symlink(&path)?;
        if fs::read(&path)? == bytes {
            return Ok(());
        }
        return Err(LabError::AlreadyExists);
    }
    write_file_atomic(root, &path, &bytes)
}

fn validate_persisted_evaluation(root: &Path, release: &ReleaseManifestV1) -> Result<(), LabError> {
    let path = evaluation_path(root, &release.evaluation_id)?;
    reject_symlink(&path)?;
    let bytes = fs::read(path)?;
    let evaluation: EvaluationReportV1 = serde_json::from_slice(&bytes)?;
    if evaluation_digest(&evaluation)? != release.evaluation_sha256
        || evaluation.specialist_id != release.specialist_id
        || evaluation.evaluated_boundary_sha256 != release.evaluated_boundary_sha256
        || assess_candidate_eligibility(&evaluation).verdict != ReadinessVerdictV1::Eligible
    {
        return Err(LabError::NotEligible);
    }
    Ok(())
}

pub(crate) fn write_file_atomic(
    root: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), LabError> {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = root.join(format!(".artifact.{}.{}.tmp", std::process::id(), sequence));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_private_file_mode(&mut options);
    let result = (|| -> Result<(), LabError> {
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, destination)?;
        File::open(destination.parent().ok_or(LabError::UnsafeProcessPath)?)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn project_active_release(state: &mut ReleaseStateV1, activated_at: chrono::DateTime<Utc>) {
    for manifest in state.releases.values_mut() {
        manifest.active_release_id = None;
        manifest.previous_release_id = None;
        manifest.activated_at = None;
    }
    if let Some(active_id) = state.active_release_id.clone()
        && let Some(active) = state.releases.get_mut(&active_id)
    {
        active.active_release_id = Some(active_id);
        active.previous_release_id = state.previous_release_id.clone();
        active.activated_at = Some(activated_at);
    }
}

fn validate_release(release: &ReleaseManifestV1) -> Result<(), LabError> {
    if release.schema_version != 1 {
        return Err(LabError::UnsupportedVersion);
    }
    if release.active_release_id.is_some()
        || release.previous_release_id.is_some()
        || release.activated_at.is_some()
    {
        return Err(LabError::InvalidContract(
            "new release contains activation state",
        ));
    }
    match release.readiness_verdict {
        ReadinessVerdictV1::Eligible if release.failure_reason.is_none() => Ok(()),
        ReadinessVerdictV1::Eligible => Err(LabError::InvalidContract(
            "eligible release has failure reason",
        )),
        ReadinessVerdictV1::NotEligible | ReadinessVerdictV1::Inconclusive
            if release
                .failure_reason
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()) =>
        {
            Ok(())
        }
        _ => Err(LabError::InvalidContract(
            "ineligible release needs failure reason",
        )),
    }
}

fn write_registry_atomic(root: &Path, registry: &RegistryV1) -> Result<(), LabError> {
    let bytes = serde_json::to_vec_pretty(registry)?;
    if bytes.len() as u64 > MAX_REGISTRY_BYTES {
        return Err(LabError::InputTooLarge);
    }
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = root.join(format!(".registry.{}.{}.tmp", std::process::id(), sequence));
    reject_symlink_if_present(&temporary)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_private_file_mode(&mut options);
    let result = (|| -> Result<(), LabError> {
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, root.join("registry.json"))?;
        File::open(root)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn read_registry(path: &Path) -> Result<RegistryV1, LabError> {
    reject_symlink(path)?;
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_REGISTRY_BYTES {
        return Err(LabError::InputTooLarge);
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

pub(crate) struct RegistryLock {
    path: PathBuf,
    file: File,
    nonce: String,
}

impl RegistryLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, LabError> {
        Self::acquire_with_stale_after(root, LOCK_STALE_AFTER)
    }

    fn acquire_with_stale_after(root: &Path, stale_after: Duration) -> Result<Self, LabError> {
        let path = root.join("registry.lock");
        for _ in 0..3 {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            set_private_file_mode(&mut options);
            match options.open(&path) {
                Ok(file) => {
                    let nonce = format!(
                        "{}-{}-{}",
                        std::process::id(),
                        unix_millis(),
                        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
                    );
                    let mut lock = Self { path, file, nonce };
                    lock.heartbeat()?;
                    return Ok(lock);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    reject_symlink(&path)?;
                    let age = fs::metadata(&path)?
                        .modified()?
                        .elapsed()
                        .unwrap_or(Duration::ZERO);
                    if age < stale_after {
                        return Err(LabError::RegistryLocked);
                    }
                    let stale = root.join(format!(
                        ".registry.lock.stale.{}.{}",
                        std::process::id(),
                        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
                    ));
                    match fs::rename(&path, &stale) {
                        Ok(()) => {
                            let _ = fs::remove_file(stale);
                        }
                        Err(rename_error)
                            if rename_error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(rename_error) => return Err(rename_error.into()),
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(LabError::RegistryLocked)
    }

    fn heartbeat(&mut self) -> Result<(), LabError> {
        self.file.seek(SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        write!(self.file, "{}\n{}\n", self.nonce, unix_millis())?;
        self.file.sync_all()?;
        Ok(())
    }
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        if fs::read_to_string(&self.path)
            .ok()
            .is_some_and(|contents| contents.lines().next() == Some(self.nonce.as_str()))
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
}

fn ensure_private_child_tree(base: &Path, components: &[&str]) -> Result<PathBuf, LabError> {
    let mut current = base.to_path_buf();
    for component in components {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(LabError::InvalidContract(
                    "private storage component is unsafe",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current)?;
            }
            Err(error) => return Err(error.into()),
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&current, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(current)
}

pub(crate) fn reject_symlink(path: &Path) -> Result<(), LabError> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(LabError::InvalidContract("symlink is not allowed"));
    }
    Ok(())
}

fn reject_existing_symlink_descendants(base: &Path, path: &Path) -> Result<(), LabError> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| LabError::InvalidContract("derived path escaped its base"))?;
    let mut current = base.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(LabError::InvalidContract("symlink escape is not allowed"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn reject_symlink_if_present(path: &Path) -> Result<(), LabError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(LabError::InvalidContract("symlink is not allowed"))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn set_private_file_mode(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvaluationMetricsV1, EvaluationReportV1, ReleaseCandidateKindV1, WorkerEventV1};
    use chrono::Utc;

    fn digest(character: char) -> Sha256Digest {
        Sha256Digest::parse(character.to_string().repeat(64)).unwrap()
    }

    fn metrics(score: f64) -> EvaluationMetricsV1 {
        EvaluationMetricsV1 {
            weighted_score: Some(score),
            schema_validity: Some(1.0),
            tool_validity: Some(1.0),
            fabricated_number_count: Some(0),
            fabricated_path_count: Some(0),
            fabricated_citation_count: Some(0),
            privacy_failure_count: Some(0),
            critical_calibration_failure_count: Some(0),
        }
    }

    fn evaluation(id: &str, score: f64) -> EvaluationReportV1 {
        EvaluationReportV1 {
            schema_version: 1,
            evaluation_id: StableId::parse(format!("evaluation-{id}")).unwrap(),
            specialist_id: StableId::parse("surrogate-experiment-reviewer").unwrap(),
            candidate_kind: ReleaseCandidateKindV1::NoTraining,
            test_dataset_state: crate::DatasetState::Frozen,
            test_dataset_sha256: digest('9'),
            untouched_base_runtime_sha256: digest('8'),
            no_training_runtime_sha256: digest('8'),
            candidate_runtime_sha256: digest('8'),
            evaluated_boundary_sha256: digest('1'),
            untouched_base: metrics(0.70),
            no_training: metrics(score),
            candidate: metrics(score),
            documented_critical_fix: None,
            created_at: Utc::now(),
        }
    }

    fn release(id: &str) -> ReleaseManifestV1 {
        ReleaseManifestV1 {
            schema_version: 1,
            specialist_id: StableId::parse("surrogate-experiment-reviewer").unwrap(),
            release_id: StableId::parse(id).unwrap(),
            base_sha256: digest('a'),
            adapter_sha256: None,
            instructions_sha256: digest('b'),
            source_sha256: digest('c'),
            dataset_sha256: digest('d'),
            tool_sha256: digest('e'),
            evaluation_id: StableId::parse("evaluation-placeholder").unwrap(),
            evaluation_sha256: digest('f'),
            readiness_verdict: ReadinessVerdictV1::Inconclusive,
            failure_reason: Some("caller value must be overwritten".to_owned()),
            active_release_id: None,
            previous_release_id: None,
            created_at: Utc::now(),
            activated_at: None,
            evaluated_boundary_sha256: digest('1'),
        }
    }

    #[test]
    fn activation_and_rollback_are_cas_atomic_and_persist_across_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = PrivateLabStore::open_at(temp.path()).unwrap();
        let specialist = StableId::parse("surrogate-experiment-reviewer").unwrap();
        let first = StableId::parse("release-001").unwrap();
        let second = StableId::parse("release-002").unwrap();

        store
            .register_release(release("release-001"), &evaluation("release-001", 0.86))
            .unwrap();
        store.activate(&specialist, &first, None).unwrap();
        store
            .register_release(release("release-002"), &evaluation("release-002", 0.87))
            .unwrap();
        store.activate(&specialist, &second, Some(&first)).unwrap();

        assert!(store.activate(&specialist, &first, Some(&first)).is_err());
        assert_eq!(
            store.release_state(&specialist).unwrap().active_release_id,
            Some(second.clone())
        );
        let state = store.release_state(&specialist).unwrap();
        assert_eq!(state.releases[&first].active_release_id, None);
        assert_eq!(state.releases[&first].activated_at, None);
        assert_eq!(
            state.releases[&second].active_release_id,
            Some(second.clone())
        );
        drop(store);

        let mut reopened = PrivateLabStore::open_at(temp.path()).unwrap();
        reopened.rollback(&specialist, Some(&second)).unwrap();
        let state = reopened.release_state(&specialist).unwrap();
        assert_eq!(state.active_release_id, Some(first));
        assert_eq!(state.previous_release_id, Some(second));
        assert_eq!(
            state.releases[&StableId::parse("release-002").unwrap()].active_release_id,
            None
        );
        assert_eq!(
            state.releases[&StableId::parse("release-002").unwrap()].activated_at,
            None
        );
    }

    #[test]
    fn activation_rejects_non_eligible_release_without_changing_registry() {
        let temp = tempfile::tempdir().unwrap();
        let mut store = PrivateLabStore::open_at(temp.path()).unwrap();
        let specialist = StableId::parse("surrogate-experiment-reviewer").unwrap();
        let release_id = StableId::parse("release-failed").unwrap();
        store
            .register_release(
                release("release-failed"),
                &evaluation("release-failed", 0.80),
            )
            .unwrap();

        assert!(store.activate(&specialist, &release_id, None).is_err());
        assert_eq!(
            store.release_state(&specialist).unwrap().active_release_id,
            None
        );
        assert_eq!(
            store.release_state(&specialist).unwrap().releases[&release_id].readiness_verdict,
            ReadinessVerdictV1::NotEligible
        );
    }

    #[test]
    fn only_typed_safe_job_events_are_appended() {
        let temp = tempfile::tempdir().unwrap();
        let store = PrivateLabStore::open_at(temp.path()).unwrap();
        let run_id = StableId::parse("run-001").unwrap();
        let event = JobEventV1 {
            schema_version: 1,
            event_id: StableId::parse("event-001").unwrap(),
            run_id: run_id.clone(),
            recorded_at: Utc::now(),
            worker_event: WorkerEventV1::Started {
                protocol_version: 1,
                run_id: run_id.clone(),
                timestamp: Utc::now(),
            },
        };
        store.append_job_event(&event).unwrap();
        let unsafe_event = JobEventV1 {
            event_id: StableId::parse("event-002").unwrap(),
            worker_event: WorkerEventV1::Failed {
                protocol_version: 1,
                run_id,
                code: "failed".to_owned(),
                safe_message: "secret path".to_owned(),
                timestamp: Utc::now(),
            },
            ..event
        };
        assert!(store.append_job_event(&unsafe_event).is_err());
    }

    #[test]
    fn stale_lock_is_recovered_and_live_lock_heartbeat_is_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let store = PrivateLabStore::open_at(temp.path()).unwrap();
        let lock_path = store.root().join("registry.lock");
        fs::write(&lock_path, "abandoned\n0\n").unwrap();
        let mut recovered =
            RegistryLock::acquire_with_stale_after(store.root(), Duration::ZERO).unwrap();
        let first = fs::read_to_string(&lock_path).unwrap();
        recovered.heartbeat().unwrap();
        let second = fs::read_to_string(&lock_path).unwrap();
        assert_eq!(first.lines().next(), second.lines().next());
        drop(recovered);
        assert!(!lock_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn store_root_is_private_and_derived_below_lab_v1() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let store = PrivateLabStore::open_at(temp.path()).unwrap();
        assert!(store.root().ends_with("lab/v1"));
        let mode = std::fs::metadata(store.root())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn derived_storage_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), temp.path().join("Kairos")).unwrap();
        assert!(PrivateLabStore::open_at(temp.path()).is_err());
    }
}
