use crate::{JobState, JobTerminalRecordV1, LabError, PrivateLabStore, Sha256Digest, StableId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const WORKER_PROTOCOL_VERSION: u16 = 1;
pub const MAX_WORKER_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_WORKER_EVENT_BYTES: usize = 16 * 1024;
const MAX_WORKER_OUTPUT_BYTES: usize = 512 * 1024;
const MAX_WORKER_STDERR_BYTES: usize = 4 * 1024;
const MAX_WORKER_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const ALLOWED_METRICS: &[(&str, &str)] = &[
    ("smoke-check", "fraction"),
    ("train-loss", "loss"),
    ("validation-loss", "loss"),
    ("tokens-per-second", "tokens-per-second"),
    ("peak-memory-bytes", "bytes"),
    ("learning-rate", "ratio"),
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOperationV1 {
    SmokeNoop,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRequestV1 {
    pub protocol_version: u16,
    pub run_id: StableId,
    pub operation: WorkerOperationV1,
    pub requested_at: DateTime<Utc>,
    pub total_iterations: u32,
}

pub fn parse_worker_request(bytes: &[u8]) -> Result<WorkerRequestV1, LabError> {
    if bytes.is_empty() || bytes.len() > MAX_WORKER_REQUEST_BYTES {
        return Err(LabError::InputTooLarge);
    }
    let request: WorkerRequestV1 = serde_json::from_slice(bytes)?;
    if request.protocol_version != WORKER_PROTOCOL_VERSION {
        return Err(LabError::UnsupportedVersion);
    }
    if request.total_iterations == 0 || request.total_iterations > 1_000 {
        return Err(LabError::InvalidContract(
            "iteration count outside scaffold bound",
        ));
    }
    Ok(request)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerArtifactKindV1 {
    Adapter,
    Metrics,
    Manifest,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerEventV1 {
    Started {
        protocol_version: u16,
        run_id: StableId,
        timestamp: DateTime<Utc>,
    },
    Progress {
        protocol_version: u16,
        run_id: StableId,
        iteration: u32,
        optimizer_updates: u32,
        total_iterations: u32,
    },
    Metric {
        protocol_version: u16,
        run_id: StableId,
        name: StableId,
        value: f64,
        unit: String,
        iteration: Option<u32>,
    },
    Artifact {
        protocol_version: u16,
        run_id: StableId,
        kind: WorkerArtifactKindV1,
        path_id: StableId,
        sha256: Sha256Digest,
    },
    Completed {
        protocol_version: u16,
        run_id: StableId,
        adapter_sha256: Option<Sha256Digest>,
        timestamp: DateTime<Utc>,
    },
    Failed {
        protocol_version: u16,
        run_id: StableId,
        code: String,
        safe_message: String,
        timestamp: DateTime<Utc>,
    },
    Cancelled {
        protocol_version: u16,
        run_id: StableId,
        timestamp: DateTime<Utc>,
    },
}

impl WorkerEventV1 {
    pub fn protocol_version(&self) -> u16 {
        match self {
            Self::Started {
                protocol_version, ..
            }
            | Self::Progress {
                protocol_version, ..
            }
            | Self::Metric {
                protocol_version, ..
            }
            | Self::Artifact {
                protocol_version, ..
            }
            | Self::Completed {
                protocol_version, ..
            }
            | Self::Failed {
                protocol_version, ..
            }
            | Self::Cancelled {
                protocol_version, ..
            } => *protocol_version,
        }
    }

    pub fn run_id(&self) -> &StableId {
        match self {
            Self::Started { run_id, .. }
            | Self::Progress { run_id, .. }
            | Self::Metric { run_id, .. }
            | Self::Artifact { run_id, .. }
            | Self::Completed { run_id, .. }
            | Self::Failed { run_id, .. }
            | Self::Cancelled { run_id, .. } => run_id,
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobEventV1 {
    pub schema_version: u16,
    pub event_id: StableId,
    pub run_id: StableId,
    pub recorded_at: DateTime<Utc>,
    pub worker_event: WorkerEventV1,
}

impl JobEventV1 {
    pub fn validate(&self) -> Result<(), LabError> {
        if self.schema_version != 1 {
            return Err(LabError::UnsupportedVersion);
        }
        validate_worker_event(&self.run_id, &self.worker_event)
    }
}

pub fn parse_worker_event_line(
    expected_run_id: &StableId,
    line: &str,
) -> Result<WorkerEventV1, LabError> {
    if line.is_empty()
        || line.len() > MAX_WORKER_EVENT_BYTES
        || line.contains('\n')
        || line.contains('\r')
    {
        return Err(LabError::InputTooLarge);
    }
    let event: WorkerEventV1 = serde_json::from_str(line)?;
    validate_worker_event(expected_run_id, &event)?;
    Ok(event)
}

fn validate_worker_event(
    expected_run_id: &StableId,
    event: &WorkerEventV1,
) -> Result<(), LabError> {
    if event.protocol_version() != WORKER_PROTOCOL_VERSION {
        return Err(LabError::UnsupportedVersion);
    }
    if event.run_id() != expected_run_id {
        return Err(LabError::WorkerProtocol("mismatched run ID"));
    }
    match event {
        WorkerEventV1::Progress {
            iteration,
            optimizer_updates,
            total_iterations,
            ..
        } if *total_iterations == 0
            || *iteration > *total_iterations
            || *optimizer_updates > *iteration =>
        {
            Err(LabError::WorkerProtocol("invalid progress counters"))
        }
        WorkerEventV1::Metric {
            name, value, unit, ..
        } if !value.is_finite()
            || !ALLOWED_METRICS.iter().any(|(allowed_name, allowed_unit)| {
                name.as_str() == *allowed_name && unit == allowed_unit
            }) =>
        {
            Err(LabError::WorkerProtocol("metric is not allowlisted"))
        }
        WorkerEventV1::Failed {
            code, safe_message, ..
        } if StableId::parse(code).is_err() || !safe_status_message(safe_message) => {
            Err(LabError::WorkerProtocol("unsafe failure event"))
        }
        _ => Ok(()),
    }
}

fn safe_status_message(message: &str) -> bool {
    if message.is_empty()
        || message.len() > 160
        || message.contains('/')
        || message.contains('\\')
        || !message.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character.is_ascii_whitespace()
                || ".,:;_-()".contains(character)
        })
    {
        return false;
    }
    let lower = message.to_ascii_lowercase();
    ![
        "secret",
        "token",
        "password",
        "prompt",
        "reasoning",
        ".env",
        "private",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[derive(Debug)]
pub struct WorkerEventJournal {
    run_id: StableId,
    started: bool,
    terminal: bool,
    events: Vec<WorkerEventV1>,
}

impl WorkerEventJournal {
    pub fn new(run_id: StableId) -> Self {
        Self {
            run_id,
            started: false,
            terminal: false,
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: WorkerEventV1) -> Result<(), LabError> {
        validate_worker_event(&self.run_id, &event)?;
        if self.terminal {
            return Err(LabError::WorkerProtocol("event after terminal event"));
        }
        match &event {
            WorkerEventV1::Started { .. } if !self.started => self.started = true,
            WorkerEventV1::Started { .. } => {
                return Err(LabError::WorkerProtocol("duplicate started event"));
            }
            _ if !self.started => {
                return Err(LabError::WorkerProtocol("first event is not started"));
            }
            _ => {}
        }
        self.terminal = event.is_terminal();
        self.events.push(event);
        Ok(())
    }

    pub fn events(&self) -> &[WorkerEventV1] {
        &self.events
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }
}

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug)]
pub struct ManagedWorkerRuntime {
    program: PathBuf,
    script: PathBuf,
    program_sha256: Sha256Digest,
    script_sha256: Sha256Digest,
}

impl ManagedWorkerRuntime {
    pub fn open_default() -> Result<Self, LabError> {
        let home =
            std::env::var_os("HOME").ok_or(LabError::InvalidContract("HOME is unavailable"))?;
        let home = fs::canonicalize(Path::new(&home)).map_err(|_| LabError::UnsafeProcessPath)?;
        let root = home.join("Library/Application Support/Kairos/lab/v1/runtime/mlx-lm-v1");
        reject_existing_symlink_descendants(&home, &root)?;
        Self::from_root(&root)
    }

    fn from_root(root: &Path) -> Result<Self, LabError> {
        let root = fs::canonicalize(root).map_err(|_| LabError::UnsafeProcessPath)?;
        let program = fixed_file_inside(&root, &root.join("python/bin/python3.12"))?;
        let script = fixed_file_inside(&root, &root.join("worker/worker.py"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&program)?.permissions().mode() & 0o111 == 0 {
                return Err(LabError::UnsafeProcessPath);
            }
        }
        Ok(Self {
            program_sha256: hash_file(&program)?,
            script_sha256: hash_file(&script)?,
            program,
            script,
        })
    }

    pub fn program_sha256(&self) -> &Sha256Digest {
        &self.program_sha256
    }

    pub fn script_sha256(&self) -> &Sha256Digest {
        &self.script_sha256
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args([self.script.as_os_str(), std::ffi::OsStr::new("--stdio")])
            .env_clear()
            .env("PYTHONNOUSERSITE", "1")
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

#[derive(Clone, Debug)]
pub struct WorkerRunner {
    runtime: ManagedWorkerRuntime,
    timeout: Duration,
}

impl WorkerRunner {
    pub fn open_default(timeout: Duration) -> Result<Self, LabError> {
        Self::from_runtime(ManagedWorkerRuntime::open_default()?, timeout)
    }

    fn from_runtime(runtime: ManagedWorkerRuntime, timeout: Duration) -> Result<Self, LabError> {
        if timeout.is_zero() || timeout > MAX_WORKER_TIMEOUT {
            return Err(LabError::InvalidContract("worker timeout is outside bound"));
        }
        Ok(Self { runtime, timeout })
    }

    pub fn run(
        &self,
        request: &WorkerRequestV1,
        cancellation: &CancellationToken,
    ) -> Result<Vec<WorkerEventV1>, LabError> {
        let request_bytes = serde_json::to_vec(request)?;
        parse_worker_request(&request_bytes)?;
        let mut child = self.runtime.command().spawn()?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or(LabError::WorkerProtocol("worker stdin unavailable"))?;
        stdin.write_all(&request_bytes)?;
        drop(stdin);

        let stdout = child
            .stdout
            .take()
            .ok_or(LabError::WorkerProtocol("worker stdout unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(LabError::WorkerProtocol("worker stderr unavailable"))?;
        let stdout_reader = thread::spawn(move || read_bounded(stdout, MAX_WORKER_OUTPUT_BYTES));
        let stderr_reader = thread::spawn(move || read_bounded(stderr, MAX_WORKER_STDERR_BYTES));
        let started = Instant::now();
        let status = loop {
            if cancellation.is_cancelled() {
                terminate_child(&mut child);
                join_reader(stdout_reader)?;
                join_reader(stderr_reader)?;
                return Err(LabError::WorkerCancelled);
            }
            if started.elapsed() >= self.timeout {
                terminate_child(&mut child);
                join_reader(stdout_reader)?;
                join_reader(stderr_reader)?;
                return Err(LabError::WorkerTimedOut);
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(5));
        };
        let stdout = join_reader(stdout_reader)?;
        let stderr = join_reader(stderr_reader)?;
        if !status.success() || !stderr.is_empty() {
            return Err(LabError::WorkerProtocol("worker exited unsuccessfully"));
        }
        let output = std::str::from_utf8(&stdout)
            .map_err(|_| LabError::WorkerProtocol("worker stdout is not UTF-8"))?;
        let mut journal = WorkerEventJournal::new(request.run_id.clone());
        for line in output.lines() {
            journal.push(parse_worker_event_line(&request.run_id, line)?)?;
        }
        if !journal.is_terminal()
            || !matches!(
                journal.events().last(),
                Some(WorkerEventV1::Completed { .. })
            )
        {
            return Err(LabError::WorkerProtocol("worker did not complete"));
        }
        Ok(journal.events)
    }
}

pub fn orchestrate_noop_run(
    store: &PrivateLabStore,
    runner: &WorkerRunner,
    request: &WorkerRequestV1,
    cancellation: &CancellationToken,
) -> Result<JobTerminalRecordV1, LabError> {
    let (state, reason_code) = match runner.run(request, cancellation) {
        Ok(events) => {
            for (index, worker_event) in events.into_iter().enumerate() {
                store.append_job_event(&JobEventV1 {
                    schema_version: 1,
                    event_id: StableId::parse(format!(
                        "{}-event-{index:03}",
                        request.run_id.as_str()
                    ))?,
                    run_id: request.run_id.clone(),
                    recorded_at: Utc::now(),
                    worker_event,
                })?;
            }
            (JobState::Succeeded, StableId::parse("completed")?)
        }
        Err(LabError::WorkerCancelled) => (JobState::Cancelled, StableId::parse("cancelled")?),
        Err(LabError::WorkerTimedOut) => (JobState::Interrupted, StableId::parse("timeout")?),
        Err(error) => {
            let record = JobTerminalRecordV1 {
                schema_version: 1,
                run_id: request.run_id.clone(),
                state: JobState::Failed,
                reason_code: StableId::parse("worker-failed")?,
                recorded_at: Utc::now(),
            };
            store.persist_job_terminal(&record)?;
            return Err(error);
        }
    };
    let record = JobTerminalRecordV1 {
        schema_version: 1,
        run_id: request.run_id.clone(),
        state,
        reason_code,
        recorded_at: Utc::now(),
    };
    store.persist_job_terminal(&record)?;
    Ok(record)
}

fn terminate_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn read_bounded(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, LabError> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if output.len() + count > limit {
            return Err(LabError::InputTooLarge);
        }
        output.extend_from_slice(&buffer[..count]);
    }
    Ok(output)
}

fn join_reader(reader: thread::JoinHandle<Result<Vec<u8>, LabError>>) -> Result<Vec<u8>, LabError> {
    reader
        .join()
        .map_err(|_| LabError::WorkerProtocol("worker reader thread failed"))?
}

fn fixed_file_inside(root: &Path, path: &Path) -> Result<PathBuf, LabError> {
    reject_symlink_components(root, path)?;
    let canonical = fs::canonicalize(path).map_err(|_| LabError::UnsafeProcessPath)?;
    if !canonical.starts_with(root) || !fs::metadata(&canonical)?.is_file() {
        return Err(LabError::UnsafeProcessPath);
    }
    Ok(canonical)
}

fn reject_existing_symlink_descendants(base: &Path, path: &Path) -> Result<(), LabError> {
    let relative = path
        .strip_prefix(base)
        .map_err(|_| LabError::UnsafeProcessPath)?;
    let mut current = base.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(LabError::UnsafeProcessPath);
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err(LabError::UnsafeProcessPath),
        }
    }
    Ok(())
}

fn reject_symlink_components(root: &Path, path: &Path) -> Result<(), LabError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| LabError::UnsafeProcessPath)?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        if fs::symlink_metadata(&current)
            .map_err(|_| LabError::UnsafeProcessPath)?
            .file_type()
            .is_symlink()
        {
            return Err(LabError::UnsafeProcessPath);
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<Sha256Digest, LabError> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Sha256Digest::parse(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn request() -> WorkerRequestV1 {
        parse_worker_request(br#"{"protocol_version":1,"run_id":"run-001","operation":"smoke_noop","requested_at":"2026-08-20T00:00:00Z","total_iterations":20}"#).unwrap()
    }

    #[test]
    fn request_parser_is_versioned_bounded_smoke_only_and_has_no_test_path() {
        let parsed = request();
        assert_eq!(parsed.run_id, StableId::parse("run-001").unwrap());
        let json = serde_json::to_string(&parsed).unwrap();
        let with_test_path = json.replace("{", "{\"test_path\":\"sealed\",");
        assert!(parse_worker_request(with_test_path.as_bytes()).is_err());
        assert!(
            parse_worker_request(json.replace("smoke_noop", "train_qlora").as_bytes()).is_err()
        );
        assert!(
            parse_worker_request(
                json.replace("\"protocol_version\":1", "\"protocol_version\":2")
                    .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn events_reject_unknown_metrics_unsafe_messages_paths_and_secrets() {
        let run = StableId::parse("run-001").unwrap();
        let started = r#"{"protocol_version":1,"type":"started","run_id":"run-001","timestamp":"2026-08-20T00:00:00Z"}"#;
        assert!(matches!(
            parse_worker_event_line(&run, started).unwrap(),
            WorkerEventV1::Started { .. }
        ));
        let unknown_metric = r#"{"protocol_version":1,"type":"metric","run_id":"run-001","name":"raw-prompt","value":1.0,"unit":"text","iteration":1}"#;
        assert!(parse_worker_event_line(&run, unknown_metric).is_err());
        let unsafe_failure = r#"{"protocol_version":1,"type":"failed","run_id":"run-001","code":"failed","safe_message":"secret at /Users/example","timestamp":"2026-08-20T00:00:00Z"}"#;
        assert!(parse_worker_event_line(&run, unsafe_failure).is_err());
        let artifact = format!(
            "{{\"protocol_version\":1,\"type\":\"artifact\",\"run_id\":\"run-001\",\"kind\":\"adapter\",\"path_id\":\"../escape\",\"sha256\":\"{}\"}}",
            "a".repeat(64)
        );
        assert!(parse_worker_event_line(&run, &artifact).is_err());
    }

    #[cfg(unix)]
    fn runtime_with_script(script: &str) -> (tempfile::TempDir, ManagedWorkerRuntime) {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("runtime");
        fs::create_dir_all(root.join("python/bin")).unwrap();
        fs::create_dir_all(root.join("worker")).unwrap();
        let program = root.join("python/bin/python3.12");
        fs::write(&program, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let worker = root.join("worker/worker.py");
        fs::write(&worker, script).unwrap();
        fs::set_permissions(&worker, fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = ManagedWorkerRuntime::from_root(&root).unwrap();
        (temp, runtime)
    }

    #[cfg(unix)]
    #[test]
    fn managed_fixed_child_correlates_run_and_completes_deterministically() {
        let script = r#"#!/bin/sh
cat >/dev/null
printf '%s\n' '{"protocol_version":1,"type":"started","run_id":"run-001","timestamp":"2026-08-20T00:00:00Z"}'
printf '%s\n' '{"protocol_version":1,"type":"completed","run_id":"run-001","adapter_sha256":null,"timestamp":"2026-08-20T00:00:00Z"}'
"#;
        let (_temp, runtime) = runtime_with_script(script);
        let runner = WorkerRunner::from_runtime(runtime, Duration::from_secs(1)).unwrap();
        let first = runner
            .run(&request(), &CancellationToken::default())
            .unwrap();
        let second = runner
            .run(&request(), &CancellationToken::default())
            .unwrap();
        assert_eq!(first, second);
        assert!(
            first
                .iter()
                .all(|event| event.run_id().as_str() == "run-001")
        );
    }

    #[cfg(unix)]
    #[test]
    fn noop_orchestration_persists_success_cancel_and_timeout_terminals() {
        let success_script = r#"#!/bin/sh
cat >/dev/null
printf '%s\n' '{"protocol_version":1,"type":"started","run_id":"run-001","timestamp":"2026-08-20T00:00:00Z"}'
printf '%s\n' '{"protocol_version":1,"type":"completed","run_id":"run-001","adapter_sha256":null,"timestamp":"2026-08-20T00:00:00Z"}'
"#;
        let (_runtime_temp, runtime) = runtime_with_script(success_script);
        let storage = tempfile::tempdir().unwrap();
        let store = PrivateLabStore::open_at(storage.path()).unwrap();
        let runner = WorkerRunner::from_runtime(runtime, Duration::from_secs(1)).unwrap();
        let completed =
            orchestrate_noop_run(&store, &runner, &request(), &CancellationToken::default())
                .unwrap();
        assert_eq!(completed.state, JobState::Succeeded);
        assert_eq!(
            store.load_job_terminal(&request().run_id).unwrap().state,
            JobState::Succeeded
        );

        let (_runtime_temp, runtime) =
            runtime_with_script("#!/bin/sh\ncat >/dev/null\nwhile :; do :; done\n");
        let storage = tempfile::tempdir().unwrap();
        let store = PrivateLabStore::open_at(storage.path()).unwrap();
        let runner = WorkerRunner::from_runtime(runtime, Duration::from_millis(25)).unwrap();
        let timed_out =
            orchestrate_noop_run(&store, &runner, &request(), &CancellationToken::default())
                .unwrap();
        assert_eq!(timed_out.state, JobState::Interrupted);

        let (_runtime_temp, runtime) =
            runtime_with_script("#!/bin/sh\ncat >/dev/null\nwhile :; do :; done\n");
        let storage = tempfile::tempdir().unwrap();
        let store = PrivateLabStore::open_at(storage.path()).unwrap();
        let runner = WorkerRunner::from_runtime(runtime, Duration::from_secs(1)).unwrap();
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        let cancelled = orchestrate_noop_run(&store, &runner, &request(), &cancellation).unwrap();
        assert_eq!(cancelled.state, JobState::Cancelled);
    }

    #[cfg(unix)]
    #[test]
    fn child_timeout_and_cancellation_are_enforced() {
        let (_temp, runtime) =
            runtime_with_script("#!/bin/sh\ncat >/dev/null\nwhile :; do :; done\n");
        let timeout_runner =
            WorkerRunner::from_runtime(runtime.clone(), Duration::from_millis(25)).unwrap();
        assert!(matches!(
            timeout_runner.run(&request(), &CancellationToken::default()),
            Err(LabError::WorkerTimedOut)
        ));

        let cancellation = CancellationToken::default();
        let trigger = cancellation.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            trigger.cancel();
        });
        let cancel_runner = WorkerRunner::from_runtime(runtime, Duration::from_secs(1)).unwrap();
        assert!(matches!(
            cancel_runner.run(&request(), &cancellation),
            Err(LabError::WorkerCancelled)
        ));
        handle.join().unwrap();
    }
}
