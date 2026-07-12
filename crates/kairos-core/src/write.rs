//! Confirmation-gated Markdown note writes for Kairos.
//!
//! The public [`NoteWriteProposal`] intentionally contains a bounded,
//! reviewable diff rather than the complete proposed note body. The complete
//! body remains in the in-memory [`WriteProposalStore`] until a caller presents
//! the matching one-time nonce. This keeps the UI from being able to forge,
//! alter, or replay a write after the preview was produced. Proposals are
//! deliberately ephemeral: restarting Kairos invalidates outstanding write
//! requests.
//!
//! This module is intentionally narrow. It supports only create and edit of
//! Markdown files inside existing, explicitly allowed directories. It never
//! creates directories, moves notes, deletes notes, follows symlinks, or
//! exposes a direct write function to the UI.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::{BrainRecord, WritePolicy};
use crate::error::{CoreError, Result};
use crate::policy::{AccessDisposition, AccessGrant, evaluate_access};

/// Default lifetime for a proposal. A fresh preview is required after it
/// expires, which avoids writes based on an old view of a note.
pub const DEFAULT_PROPOSAL_TTL_SECONDS: i64 = 10 * 60;

/// A proposal must expire within an hour, even if a caller misconfigures the
/// UI. Long-lived write capabilities are intentionally unsupported.
pub const MAX_PROPOSAL_TTL_SECONDS: i64 = 60 * 60;

/// Markdown notes are capped to keep previews and atomic replacement bounded.
pub const DEFAULT_MAX_MARKDOWN_BYTES: usize = 1_000_000;
pub const MAX_MARKDOWN_BYTES: usize = 5_000_000;

/// The default maximum preview size. This is a byte cap applied only at UTF-8
/// character boundaries, so the resulting diff is always valid text.
pub const DEFAULT_MAX_DIFF_CHARS: usize = 12_000;
pub const MIN_MAX_DIFF_CHARS: usize = 256;
pub const MAX_MAX_DIFF_CHARS: usize = 24_000;

/// A user-reviewable operation. There are deliberately no move or delete
/// variants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteWriteKind {
    Create,
    Edit,
}

/// Input used to prepare a write proposal. The complete note body is retained
/// only in the store after successful proposal creation; the resulting
/// [`NoteWriteProposal`] contains only a bounded diff excerpt.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteWriteRequest {
    pub brain_id: String,
    pub relative_path: String,
    pub kind: NoteWriteKind,
    pub markdown: String,
}

/// Caller-supplied limits for a brain's note-write surface.
///
/// `allowed_directory_prefixes` is a literal list of relative directories such
/// as `01_Daily` or `02_Projects/Kairos`; it is not a glob list. An empty list
/// is valid and intentionally permits no writes. Kairos must not make a write
/// path available merely because a brain uses `propose_confirm`. It is an
/// additional narrow scope: a path must also be present in the registered
/// brain's `write_directories` policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteProposalOptions {
    #[serde(default)]
    pub allowed_directory_prefixes: Vec<String>,
    #[serde(default = "default_proposal_ttl_seconds")]
    pub proposal_ttl_seconds: i64,
    #[serde(default = "default_max_markdown_bytes")]
    pub max_markdown_bytes: usize,
    #[serde(default = "default_max_diff_chars")]
    pub max_diff_chars: usize,
}

impl Default for WriteProposalOptions {
    fn default() -> Self {
        Self {
            // Fails closed until a future per-brain setting explicitly grants
            // a directory. This prevents an arbitrary write surface by
            // default, even for a ProposeConfirm brain.
            allowed_directory_prefixes: Vec::new(),
            proposal_ttl_seconds: DEFAULT_PROPOSAL_TTL_SECONDS,
            max_markdown_bytes: DEFAULT_MAX_MARKDOWN_BYTES,
            max_diff_chars: DEFAULT_MAX_DIFF_CHARS,
        }
    }
}

impl WriteProposalOptions {
    /// Build the normal per-brain scope from the registered policy. Native UI
    /// code can further narrow this list for a particular flow, but cannot use
    /// this type to broaden a brain's persisted `write_directories` setting.
    pub fn for_brain(brain: &BrainRecord) -> Self {
        Self {
            allowed_directory_prefixes: brain.write_directories.clone(),
            ..Self::default()
        }
    }

    /// Validate explicit caller settings before a proposal is issued.
    pub fn validate(&self) -> Result<()> {
        if !(1..=MAX_PROPOSAL_TTL_SECONDS).contains(&self.proposal_ttl_seconds) {
            return Err(CoreError::InvalidPath(format!(
                "proposal TTL must be between 1 and {MAX_PROPOSAL_TTL_SECONDS} seconds"
            )));
        }
        if self.max_markdown_bytes == 0 || self.max_markdown_bytes > MAX_MARKDOWN_BYTES {
            return Err(CoreError::InvalidPath(format!(
                "Markdown write size must be between 1 and {MAX_MARKDOWN_BYTES} bytes"
            )));
        }
        if !(MIN_MAX_DIFF_CHARS..=MAX_MAX_DIFF_CHARS).contains(&self.max_diff_chars) {
            return Err(CoreError::InvalidPath(format!(
                "diff preview limit must be between {MIN_MAX_DIFF_CHARS} and {MAX_MAX_DIFF_CHARS} characters"
            )));
        }
        for prefix in &self.allowed_directory_prefixes {
            let _ = normalize_directory_prefix(prefix)?;
        }
        Ok(())
    }

    fn allows_relative_path(&self, relative_path: &str) -> Result<bool> {
        allowed_by_directory_prefixes(relative_path, &self.allowed_directory_prefixes)
    }
}

fn default_proposal_ttl_seconds() -> i64 {
    DEFAULT_PROPOSAL_TTL_SECONDS
}

fn default_max_markdown_bytes() -> usize {
    DEFAULT_MAX_MARKDOWN_BYTES
}

fn default_max_diff_chars() -> usize {
    DEFAULT_MAX_DIFF_CHARS
}

/// A preview the UI can display before requesting a confirmation. It omits the
/// complete proposed Markdown body by design; [`WriteProposalStore`] keeps that
/// body private until confirmation, while `diff` contains bounded changed-line
/// excerpts for user review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteWriteProposal {
    pub id: String,
    pub nonce: String,
    pub brain_id: String,
    pub relative_path: String,
    pub kind: NoteWriteKind,
    /// `None` means the proposal expects the file to be absent (a create).
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub diff: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// The only UI input accepted by [`WriteProposalStore::confirm`]. The nonce is
/// one-time and is checked against the server-side pending proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteWriteConfirmation {
    pub proposal_id: String,
    pub nonce: String,
}

/// Audit-friendly result returned after an atomic write completes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedNoteWrite {
    pub proposal_id: String,
    pub brain_id: String,
    pub relative_path: String,
    pub kind: NoteWriteKind,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub written_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct PendingNoteWrite {
    proposal: NoteWriteProposal,
    markdown: String,
}

/// In-memory, one-time proposal registry. Hold this behind the desktop app's
/// normal synchronization primitive (for example `Mutex<WriteProposalStore>`)
/// so a confirmation cannot be processed twice concurrently.
#[derive(Debug, Default)]
pub struct WriteProposalStore {
    pending: BTreeMap<String, PendingNoteWrite>,
}

impl WriteProposalStore {
    /// Number of currently pending proposals. Intended for diagnostics and
    /// tests; pending Markdown itself remains private to this module.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Return the registered brain that owns a pending proposal without
    /// exposing its Markdown body. Native callers use this to select the one
    /// policy record that must re-authorize confirmation, rather than trying
    /// every connected brain and losing a useful conflict or expiry error.
    pub fn proposal_brain_id(&self, proposal_id: &str) -> Option<&str> {
        self.pending
            .get(proposal_id)
            .map(|pending| pending.proposal.brain_id.as_str())
    }

    /// Discard expired proposals and return how many were removed.
    pub fn purge_expired(&mut self) -> usize {
        self.purge_expired_at(Utc::now())
    }

    /// Testable form of [`Self::purge_expired`].
    pub fn purge_expired_at(&mut self, now: DateTime<Utc>) -> usize {
        let before = self.pending.len();
        self.pending
            .retain(|_, pending| pending.proposal.expires_at > now);
        before.saturating_sub(self.pending.len())
    }

    /// Prepare a new confirmation-gated note write using the current time.
    pub fn propose(
        &mut self,
        brain: &BrainRecord,
        request: NoteWriteRequest,
        options: &WriteProposalOptions,
        grants: &[AccessGrant],
    ) -> Result<NoteWriteProposal> {
        self.propose_at(brain, request, options, grants, Utc::now())
    }

    /// Prepare a new confirmation-gated note write at an explicit time.
    ///
    /// This version exists so callers can write deterministic tests. Production
    /// code should normally call [`Self::propose`].
    pub fn propose_at(
        &mut self,
        brain: &BrainRecord,
        request: NoteWriteRequest,
        options: &WriteProposalOptions,
        grants: &[AccessGrant],
        now: DateTime<Utc>,
    ) -> Result<NoteWriteProposal> {
        self.purge_expired_at(now);
        validate_write_request(brain, &request, options, grants)?;

        let (relative_path, normalized_path) =
            normalize_markdown_relative_path(&request.relative_path)?;
        let target = resolve_target(brain, &relative_path, &normalized_path)?;
        let current_markdown = current_markdown(&target, &request.kind)?;

        if let Some(current) = current_markdown.as_deref() {
            ensure_access(brain, &normalized_path, current, grants)?;
        }
        ensure_access(brain, &normalized_path, &request.markdown, grants)?;

        if request.kind == NoteWriteKind::Edit
            && current_markdown
                .as_deref()
                .is_some_and(|current| current == request.markdown)
        {
            return Err(CoreError::InvalidPath(
                "an edit proposal must change the existing Markdown note".to_owned(),
            ));
        }

        let proposal_id = self.next_id()?;
        let proposal = NoteWriteProposal {
            id: proposal_id.clone(),
            nonce: random_hex(32)?,
            brain_id: brain.id.clone(),
            relative_path: normalized_path,
            kind: request.kind,
            before_sha256: current_markdown.as_deref().map(markdown_sha256),
            after_sha256: markdown_sha256(&request.markdown),
            diff: bounded_markdown_diff(
                current_markdown.as_deref(),
                &request.markdown,
                options.max_diff_chars,
            ),
            created_at: now,
            expires_at: now + Duration::seconds(options.proposal_ttl_seconds),
        };

        self.pending.insert(
            proposal_id,
            PendingNoteWrite {
                proposal: proposal.clone(),
                markdown: request.markdown,
            },
        );
        Ok(proposal)
    }

    /// Confirm a stored proposal using the current time. Confirmation consumes
    /// the proposal before the filesystem is touched, so a failed write needs a
    /// fresh preview instead of allowing a stale proposal to be replayed.
    pub fn confirm(
        &mut self,
        brain: &BrainRecord,
        confirmation: &NoteWriteConfirmation,
        options: &WriteProposalOptions,
        grants: &[AccessGrant],
    ) -> Result<ConfirmedNoteWrite> {
        self.confirm_at(brain, confirmation, options, grants, Utc::now())
    }

    /// Testable form of [`Self::confirm`]. It revalidates both policy and the
    /// file's SHA-256 immediately before writing.
    pub fn confirm_at(
        &mut self,
        brain: &BrainRecord,
        confirmation: &NoteWriteConfirmation,
        options: &WriteProposalOptions,
        grants: &[AccessGrant],
        now: DateTime<Utc>,
    ) -> Result<ConfirmedNoteWrite> {
        let pending = self
            .pending
            .get(&confirmation.proposal_id)
            .cloned()
            .ok_or_else(|| {
                CoreError::PolicyDenied("write proposal is missing or expired".to_owned())
            })?;

        if !constant_time_eq(
            pending.proposal.nonce.as_bytes(),
            confirmation.nonce.as_bytes(),
        ) {
            return Err(CoreError::PolicyDenied(
                "write proposal nonce does not match".to_owned(),
            ));
        }
        if pending.proposal.expires_at <= now {
            self.pending.remove(&confirmation.proposal_id);
            return Err(CoreError::PolicyDenied(
                "write proposal has expired".to_owned(),
            ));
        }
        // Leave the candidate intact until its expiry is reported clearly, but
        // prune unrelated stale entries while this store is already mutable.
        self.purge_expired_at(now);
        if pending.proposal.brain_id != brain.id {
            return Err(CoreError::PolicyDenied(
                "write proposal belongs to a different brain".to_owned(),
            ));
        }

        // One successful nonce presentation consumes the proposal before the
        // write begins. This makes confirmation one-time even if a subsequent
        // filesystem error requires the user to ask Kairos for a fresh plan.
        self.pending.remove(&confirmation.proposal_id);

        let request = NoteWriteRequest {
            brain_id: pending.proposal.brain_id.clone(),
            relative_path: pending.proposal.relative_path.clone(),
            kind: pending.proposal.kind.clone(),
            markdown: pending.markdown.clone(),
        };
        validate_write_request(brain, &request, options, grants)?;

        let (relative_path, normalized_path) =
            normalize_markdown_relative_path(&pending.proposal.relative_path)?;
        let target = resolve_target(brain, &relative_path, &normalized_path)?;
        let current_markdown = current_markdown(&target, &pending.proposal.kind)?;

        if current_markdown.as_deref().map(markdown_sha256) != pending.proposal.before_sha256 {
            return Err(CoreError::PolicyDenied(format!(
                "write conflict: {} changed after the proposal was previewed",
                pending.proposal.relative_path
            )));
        }
        if markdown_sha256(&pending.markdown) != pending.proposal.after_sha256 {
            return Err(CoreError::PolicyDenied(
                "stored write proposal failed its content-integrity check".to_owned(),
            ));
        }

        if let Some(current) = current_markdown.as_deref() {
            ensure_access(brain, &normalized_path, current, grants)?;
        }
        ensure_access(brain, &normalized_path, &pending.markdown, grants)?;

        match pending.proposal.kind {
            NoteWriteKind::Create => atomic_create(&target, &pending.markdown)?,
            NoteWriteKind::Edit => atomic_replace(&target, &pending.markdown)?,
        }

        Ok(ConfirmedNoteWrite {
            proposal_id: pending.proposal.id,
            brain_id: pending.proposal.brain_id,
            relative_path: pending.proposal.relative_path,
            kind: pending.proposal.kind,
            before_sha256: pending.proposal.before_sha256,
            after_sha256: pending.proposal.after_sha256,
            written_at: now,
        })
    }

    fn next_id(&self) -> Result<String> {
        // The entropy source supplies an opaque identifier rather than a
        // sequential handle that a WebView could guess.
        for _ in 0..8 {
            let id = random_hex(16)?;
            if !self.pending.contains_key(&id) {
                return Ok(id);
            }
        }
        Err(CoreError::PolicyDenied(
            "could not allocate a unique write proposal identifier".to_owned(),
        ))
    }
}

/// SHA-256 used to bind the reviewed before/after Markdown states.
pub fn markdown_sha256(markdown: &str) -> String {
    format!("{:x}", Sha256::digest(markdown.as_bytes()))
}

fn validate_write_request(
    brain: &BrainRecord,
    request: &NoteWriteRequest,
    options: &WriteProposalOptions,
    grants: &[AccessGrant],
) -> Result<()> {
    if !brain.enabled {
        return Err(CoreError::BrainDisabled(brain.id.clone()));
    }
    if brain.write_policy != WritePolicy::ProposeConfirm {
        return Err(CoreError::PolicyDenied(
            "this brain does not permit confirmed note writes".to_owned(),
        ));
    }
    if request.brain_id != brain.id {
        return Err(CoreError::PolicyDenied(
            "write request brain ID does not match the registered brain".to_owned(),
        ));
    }
    options.validate()?;
    let (_, normalized_path) = normalize_markdown_relative_path(&request.relative_path)?;
    if !allowed_by_directory_prefixes(&normalized_path, &brain.write_directories)? {
        return Err(CoreError::PolicyDenied(
            "this path is outside the brain's registered write directories".to_owned(),
        ));
    }
    if !options.allows_relative_path(&normalized_path)? {
        return Err(CoreError::PolicyDenied(
            "this path is outside Kairos's explicitly allowed write directories".to_owned(),
        ));
    }
    if request.markdown.len() > options.max_markdown_bytes {
        return Err(CoreError::PolicyDenied(format!(
            "Markdown exceeds this brain's {} byte write limit",
            options.max_markdown_bytes
        )));
    }
    if request.markdown.contains('\0') {
        return Err(CoreError::InvalidPath(
            "Markdown note content cannot contain NUL bytes".to_owned(),
        ));
    }

    // Check deny and explicit-only *path* rules before resolving or reading a
    // target. Content-level private markers are checked after the current note
    // is safely read, and the proposed content is checked below by callers.
    ensure_access(brain, &normalized_path, "", grants)
}

fn ensure_access(
    brain: &BrainRecord,
    relative_path: &str,
    markdown: &str,
    grants: &[AccessGrant],
) -> Result<()> {
    match evaluate_access(brain, relative_path, markdown, grants)? {
        AccessDisposition::Allowed => Ok(()),
        AccessDisposition::ExplicitRequestRequired => Err(CoreError::PolicyDenied(format!(
            "{} requires an explicit access grant before Kairos can write it",
            relative_path
        ))),
        AccessDisposition::Denied => Err(CoreError::PolicyDenied(format!(
            "{} is denied by this brain's policy",
            relative_path
        ))),
    }
}

fn normalize_markdown_relative_path(input: &str) -> Result<(PathBuf, String)> {
    if input.is_empty() || input.contains('\\') || input.contains('\0') {
        return Err(CoreError::InvalidPath(
            "note path must be a non-empty slash-delimited relative path".to_owned(),
        ));
    }
    let path = Path::new(input);
    if path.is_absolute() {
        return Err(CoreError::InvalidPath(
            "note path must be relative to its registered brain".to_owned(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(CoreError::InvalidPath(format!(
                    "unsafe note path component in {input:?}"
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty()
        || normalized
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("md")
    {
        return Err(CoreError::PolicyDenied(
            "Kairos only creates or edits .md Markdown notes".to_owned(),
        ));
    }
    let normalized_string = normalized
        .to_str()
        .ok_or_else(|| CoreError::InvalidPath("note path is not valid UTF-8".to_owned()))?
        .replace('\\', "/");
    Ok((normalized, normalized_string))
}

fn normalize_directory_prefix(input: &str) -> Result<String> {
    if input.is_empty() || input.contains('\\') || input.contains('\0') {
        return Err(CoreError::InvalidPath(
            "write directory prefixes must be non-empty relative paths".to_owned(),
        ));
    }
    if input
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | ']' | '{' | '}'))
    {
        return Err(CoreError::InvalidPath(
            "write directory prefixes are literal paths, not globs".to_owned(),
        ));
    }
    let path = Path::new(input);
    if path.is_absolute() {
        return Err(CoreError::InvalidPath(
            "write directory prefixes must be relative".to_owned(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(segment) => normalized.push(segment),
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(CoreError::InvalidPath(format!(
                    "unsafe write directory prefix in {input:?}"
                )));
            }
        }
    }
    normalized
        .to_str()
        .map(|path| path.replace('\\', "/"))
        .ok_or_else(|| CoreError::InvalidPath("write directory is not valid UTF-8".to_owned()))
}

fn allowed_by_directory_prefixes(relative_path: &str, prefixes: &[String]) -> Result<bool> {
    let (_, normalized_path) = normalize_markdown_relative_path(relative_path)?;
    for prefix in prefixes {
        let normalized_prefix = normalize_directory_prefix(prefix)?;
        if normalized_path
            .strip_prefix(&normalized_prefix)
            .is_some_and(|remaining| remaining.starts_with('/'))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A resolved non-symlink file target contained by a canonical brain root.
#[derive(Debug)]
struct ResolvedTarget {
    path: PathBuf,
}

fn resolve_target(
    brain: &BrainRecord,
    relative_path: &Path,
    normalized_path: &str,
) -> Result<ResolvedTarget> {
    let root = fs::canonicalize(&brain.root_path).map_err(|error| {
        CoreError::InvalidPath(format!(
            "cannot access registered brain root {}: {error}",
            brain.root_path.display()
        ))
    })?;
    if !fs::metadata(&root)?.is_dir() {
        return Err(CoreError::InvalidPath(format!(
            "registered brain root is not a directory: {}",
            root.display()
        )));
    }

    // `relative_path` was normalized before entry. Walk its parent components
    // with symlink_metadata so neither an ancestor nor the target itself can
    // redirect a write outside the canonical registered root.
    let mut cursor = root.clone();
    let mut components = relative_path.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(segment) = component else {
            return Err(CoreError::InvalidPath(format!(
                "unsafe note path component in {normalized_path:?}"
            )));
        };
        let next = cursor.join(segment);
        if components.peek().is_some() {
            let metadata = fs::symlink_metadata(&next).map_err(|error| {
                CoreError::InvalidPath(format!(
                    "parent directory does not exist for {normalized_path}: {error}"
                ))
            })?;
            if metadata.file_type().is_symlink() {
                return Err(CoreError::PolicyDenied(format!(
                    "symlinked parent directory is not writable: {}",
                    next.display()
                )));
            }
            if !metadata.is_dir() {
                return Err(CoreError::InvalidPath(format!(
                    "parent path is not a directory: {}",
                    next.display()
                )));
            }
            cursor = next;
            continue;
        }

        match fs::symlink_metadata(&next) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(CoreError::PolicyDenied(format!(
                        "symlinked Markdown target is not writable: {}",
                        next.display()
                    )));
                }
                if !metadata.is_file() {
                    return Err(CoreError::InvalidPath(format!(
                        "Markdown target is not a regular file: {}",
                        next.display()
                    )));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        if !next.starts_with(&root) {
            // This should be impossible after normalized components and the
            // symlink walk, but retain a fail-closed containment check.
            return Err(CoreError::PolicyDenied(format!(
                "note path escapes the registered brain root: {}",
                next.display()
            )));
        }
        return Ok(ResolvedTarget { path: next });
    }

    Err(CoreError::InvalidPath(
        "note path must name a Markdown file".to_owned(),
    ))
}

fn current_markdown(target: &ResolvedTarget, kind: &NoteWriteKind) -> Result<Option<String>> {
    match fs::read_to_string(&target.path) {
        Ok(markdown) => match kind {
            NoteWriteKind::Create => Err(CoreError::PolicyDenied(format!(
                "write conflict: {} already exists",
                target.path.display()
            ))),
            NoteWriteKind::Edit => Ok(Some(markdown)),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => match kind {
            NoteWriteKind::Create => Ok(None),
            NoteWriteKind::Edit => Err(CoreError::PolicyDenied(format!(
                "write conflict: {} no longer exists",
                target.path.display()
            ))),
        },
        Err(error) => Err(error.into()),
    }
}

fn random_hex(byte_count: usize) -> Result<String> {
    let mut bytes = vec![0_u8; byte_count];
    // Kairos's desktop alpha is macOS-first. Failing closed is safer than
    // minting a predictable confirmation capability on a platform without the
    // system entropy device.
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut output = String::with_capacity(byte_count * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    Ok(output)
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

fn atomic_create(target: &ResolvedTarget, markdown: &str) -> Result<()> {
    let (temporary_path, mut temporary_file) = create_temporary_file(target.path.parent())?;
    let result = (|| -> Result<()> {
        temporary_file.write_all(markdown.as_bytes())?;
        temporary_file.sync_all()?;
        drop(temporary_file);

        // Unlike rename, hard_link never replaces an existing destination.
        // That preserves create's absent-file precondition even if another
        // process creates the target between conflict detection and commit.
        match fs::hard_link(&temporary_path, &target.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(CoreError::PolicyDenied(format!(
                    "write conflict: {} was created while confirmation was pending",
                    target.path.display()
                )));
            }
            Err(error) => return Err(error.into()),
        }
        sync_directory(target.path.parent());
        Ok(())
    })();
    let _ = fs::remove_file(&temporary_path);
    result
}

fn atomic_replace(target: &ResolvedTarget, markdown: &str) -> Result<()> {
    let existing_metadata = fs::symlink_metadata(&target.path)?;
    if existing_metadata.file_type().is_symlink() || !existing_metadata.is_file() {
        return Err(CoreError::PolicyDenied(format!(
            "Markdown target changed into an unsafe path before commit: {}",
            target.path.display()
        )));
    }
    let existing_permissions = existing_metadata.permissions();
    let (temporary_path, mut temporary_file) = create_temporary_file(target.path.parent())?;
    let result = (|| -> Result<()> {
        temporary_file.write_all(markdown.as_bytes())?;
        temporary_file.sync_all()?;
        drop(temporary_file);
        // Preserve the note's existing permission mode. The temporary file is
        // private while it is being written, then receives the original mode
        // immediately before an atomic same-directory rename.
        fs::set_permissions(&temporary_path, existing_permissions)?;
        fs::rename(&temporary_path, &target.path)?;
        sync_directory(target.path.parent());
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn create_temporary_file(parent: Option<&Path>) -> Result<(PathBuf, File)> {
    let parent = parent.ok_or_else(|| {
        CoreError::InvalidPath("Markdown target has no parent directory".to_owned())
    })?;
    for _ in 0..8 {
        let path = parent.join(format!(".kairos-write-{}.tmp", random_hex(16)?));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(CoreError::PolicyDenied(
        "could not allocate a private temporary Markdown file".to_owned(),
    ))
}

fn sync_directory(parent: Option<&Path>) {
    if let Some(parent) = parent
        && let Ok(directory) = File::open(parent)
    {
        let _ = directory.sync_all();
    }
}

fn bounded_markdown_diff(before: Option<&str>, after: &str, maximum: usize) -> String {
    let before_lines: Vec<&str> = before.map_or_else(Vec::new, |text| text.lines().collect());
    let after_lines: Vec<&str> = after.lines().collect();

    let mut prefix = 0;
    while prefix < before_lines.len()
        && prefix < after_lines.len()
        && before_lines[prefix] == after_lines[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < before_lines.len().saturating_sub(prefix)
        && suffix < after_lines.len().saturating_sub(prefix)
        && before_lines[before_lines.len() - suffix - 1]
            == after_lines[after_lines.len() - suffix - 1]
    {
        suffix += 1;
    }

    let before_end = before_lines.len().saturating_sub(suffix);
    let after_end = after_lines.len().saturating_sub(suffix);
    let mut diff = String::new();
    let mut complete = true;

    complete &= append_bounded(
        &mut diff,
        if before.is_some() {
            "--- current Markdown\n"
        } else {
            "--- /dev/null\n"
        },
        maximum,
    );
    complete &= append_bounded(&mut diff, "+++ proposed Markdown\n", maximum);
    complete &= append_bounded(
        &mut diff,
        &format!(
            "@@ -{} lines +{} lines @@\n",
            before_lines.len(),
            after_lines.len()
        ),
        maximum,
    );
    if prefix > 0 {
        complete &= append_bounded(
            &mut diff,
            &format!("  … {prefix} unchanged line(s) …\n"),
            maximum,
        );
    }
    for line in &before_lines[prefix..before_end] {
        complete &= append_bounded(
            &mut diff,
            &format!("-{}\n", render_diff_line(line)),
            maximum,
        );
        if !complete {
            break;
        }
    }
    if complete {
        for line in &after_lines[prefix..after_end] {
            complete &= append_bounded(
                &mut diff,
                &format!("+{}\n", render_diff_line(line)),
                maximum,
            );
            if !complete {
                break;
            }
        }
    }
    if complete && suffix > 0 {
        complete &= append_bounded(
            &mut diff,
            &format!("  … {suffix} unchanged line(s) …\n"),
            maximum,
        );
    }
    if !complete {
        append_truncation_marker(&mut diff, maximum);
    }
    diff
}

fn render_diff_line(line: &str) -> String {
    let mut rendered = String::new();
    for character in line.chars() {
        match character {
            '\t' => rendered.push_str("    "),
            character if character.is_control() => rendered.push('�'),
            character => rendered.push(character),
        }
        if rendered.len() >= 800 {
            truncate_utf8(&mut rendered, 796);
            rendered.push_str(" …");
            break;
        }
    }
    rendered
}

fn append_bounded(output: &mut String, addition: &str, maximum: usize) -> bool {
    if output.len() >= maximum {
        return false;
    }
    if output.len() + addition.len() <= maximum {
        output.push_str(addition);
        return true;
    }
    let remaining = maximum.saturating_sub(output.len());
    let mut end = remaining;
    while end > 0 && !addition.is_char_boundary(end) {
        end -= 1;
    }
    output.push_str(&addition[..end]);
    false
}

fn append_truncation_marker(output: &mut String, maximum: usize) {
    const MARKER: &str = "\n… diff truncated by Kairos …\n";
    if maximum < MARKER.len() {
        truncate_utf8(output, maximum);
        return;
    }
    let available_before_marker = maximum - MARKER.len();
    truncate_utf8(output, available_before_marker);
    output.push_str(MARKER);
}

fn truncate_utf8(value: &mut String, maximum: usize) {
    if value.len() <= maximum {
        return;
    }
    let mut end = maximum;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::config::{BrainRole, EgressPolicy, ReadPolicy};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "kairos-write-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn write(&self, relative_path: &str, markdown: &str) {
            let path = self.path.join(relative_path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, markdown).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn brain(root: PathBuf) -> BrainRecord {
        BrainRecord {
            id: "everyday".to_owned(),
            name: "Everyday".to_owned(),
            role: BrainRole::Everyday,
            root_path: root,
            router_paths: Vec::new(),
            context_paths: Vec::new(),
            routing_hints: Vec::new(),
            enabled: true,
            read_policy: ReadPolicy {
                explicit_only_patterns: vec!["90_Private/**".to_owned()],
                deny_patterns: vec!["Denied/**".to_owned()],
                ..Default::default()
            },
            egress_policy: EgressPolicy::LocalOnly,
            write_policy: WritePolicy::ProposeConfirm,
            write_directories: vec!["Notes".to_owned(), "90_Private".to_owned()],
            graph_enabled: true,
        }
    }

    fn options() -> WriteProposalOptions {
        WriteProposalOptions {
            allowed_directory_prefixes: vec!["Notes".to_owned(), "90_Private".to_owned()],
            proposal_ttl_seconds: 60,
            max_markdown_bytes: 8_000,
            max_diff_chars: 256,
        }
    }

    fn request(kind: NoteWriteKind, path: &str, markdown: &str) -> NoteWriteRequest {
        NoteWriteRequest {
            brain_id: "everyday".to_owned(),
            relative_path: path.to_owned(),
            kind,
            markdown: markdown.to_owned(),
        }
    }

    fn confirmation(proposal: &NoteWriteProposal) -> NoteWriteConfirmation {
        NoteWriteConfirmation {
            proposal_id: proposal.id.clone(),
            nonce: proposal.nonce.clone(),
        }
    }

    fn grant(path: &str) -> AccessGrant {
        AccessGrant {
            brain_id: "everyday".to_owned(),
            relative_path: path.to_owned(),
            purpose: "user explicitly approved this note".to_owned(),
        }
    }

    #[test]
    fn creates_markdown_only_after_valid_confirmation() {
        let temp = TempDir::new("create");
        fs::create_dir_all(temp.path.join("Notes")).unwrap();
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();

        let proposal = store
            .propose(
                &brain,
                request(NoteWriteKind::Create, "Notes/New.md", "# New\n\nBody\n"),
                &options(),
                &[],
            )
            .unwrap();
        assert_eq!(proposal.before_sha256, None);
        assert_eq!(proposal.after_sha256, markdown_sha256("# New\n\nBody\n"));
        assert!(proposal.diff.contains("+++ proposed Markdown"));
        assert!(
            !serde_json::to_string(&proposal)
                .unwrap()
                .contains("\"markdown\"")
        );
        assert_eq!(store.len(), 1);

        let confirmed = store
            .confirm(&brain, &confirmation(&proposal), &options(), &[])
            .unwrap();
        assert_eq!(confirmed.after_sha256, proposal.after_sha256);
        assert_eq!(
            fs::read_to_string(temp.path.join("Notes/New.md")).unwrap(),
            "# New\n\nBody\n"
        );
        assert!(store.is_empty());
        assert!(fs::read_dir(temp.path.join("Notes")).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".kairos-write-")
        }));
    }

    #[test]
    fn edits_atomically_and_binds_the_before_hash() {
        let temp = TempDir::new("edit");
        temp.write("Notes/Today.md", "# Today\nold task\n");
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();

        let proposal = store
            .propose(
                &brain,
                request(NoteWriteKind::Edit, "Notes/Today.md", "# Today\nnew task\n"),
                &options(),
                &[],
            )
            .unwrap();
        let old_hash = markdown_sha256("# Today\nold task\n");
        assert_eq!(proposal.before_sha256.as_deref(), Some(old_hash.as_str()));
        assert!(proposal.diff.contains("-old task"));
        assert!(proposal.diff.contains("+new task"));

        store
            .confirm(&brain, &confirmation(&proposal), &options(), &[])
            .unwrap();
        assert_eq!(
            fs::read_to_string(temp.path.join("Notes/Today.md")).unwrap(),
            "# Today\nnew task\n"
        );
    }

    #[test]
    fn rejects_stale_edit_without_overwriting_external_change() {
        let temp = TempDir::new("conflict");
        temp.write("Notes/Today.md", "old\n");
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();
        let proposal = store
            .propose(
                &brain,
                request(NoteWriteKind::Edit, "Notes/Today.md", "Kairos version\n"),
                &options(),
                &[],
            )
            .unwrap();

        temp.write("Notes/Today.md", "external version\n");
        let error = store
            .confirm(&brain, &confirmation(&proposal), &options(), &[])
            .unwrap_err();
        assert!(error.to_string().contains("write conflict"));
        assert_eq!(
            fs::read_to_string(temp.path.join("Notes/Today.md")).unwrap(),
            "external version\n"
        );
        assert!(store.is_empty(), "a presented proposal is one-time");
    }

    #[test]
    fn create_detects_a_file_created_after_preview() {
        let temp = TempDir::new("create-conflict");
        fs::create_dir_all(temp.path.join("Notes")).unwrap();
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();
        let proposal = store
            .propose(
                &brain,
                request(NoteWriteKind::Create, "Notes/New.md", "Kairos\n"),
                &options(),
                &[],
            )
            .unwrap();

        temp.write("Notes/New.md", "external\n");
        let error = store
            .confirm(&brain, &confirmation(&proposal), &options(), &[])
            .unwrap_err();
        assert!(error.to_string().contains("write conflict"));
        assert_eq!(
            fs::read_to_string(temp.path.join("Notes/New.md")).unwrap(),
            "external\n"
        );
    }

    #[test]
    fn rejects_expired_or_wrong_nonce_without_writing() {
        let temp = TempDir::new("expiry");
        fs::create_dir_all(temp.path.join("Notes")).unwrap();
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();
        let now = Utc::now();
        let proposal = store
            .propose_at(
                &brain,
                request(NoteWriteKind::Create, "Notes/New.md", "body"),
                &options(),
                &[],
                now,
            )
            .unwrap();

        let wrong_nonce = NoteWriteConfirmation {
            proposal_id: proposal.id.clone(),
            nonce: "wrong".to_owned(),
        };
        assert!(
            store
                .confirm_at(&brain, &wrong_nonce, &options(), &[], now)
                .unwrap_err()
                .to_string()
                .contains("nonce")
        );
        assert!(
            store.len() == 1,
            "a bad nonce must not cancel the valid preview"
        );

        let error = store
            .confirm_at(
                &brain,
                &confirmation(&proposal),
                &options(),
                &[],
                now + Duration::seconds(options().proposal_ttl_seconds),
            )
            .unwrap_err();
        assert!(error.to_string().contains("has expired"));
        assert!(!temp.path.join("Notes/New.md").exists());
        assert!(store.is_empty());
    }

    #[test]
    fn fails_closed_for_write_policy_scope_and_unsafe_paths() {
        let temp = TempDir::new("policy");
        fs::create_dir_all(temp.path.join("Notes")).unwrap();
        fs::create_dir_all(temp.path.join("Elsewhere")).unwrap();
        let mut brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();

        brain.write_policy = WritePolicy::ReadOnly;
        assert!(
            store
                .propose(
                    &brain,
                    request(NoteWriteKind::Create, "Notes/No.md", "body"),
                    &options(),
                    &[],
                )
                .unwrap_err()
                .to_string()
                .contains("does not permit")
        );

        brain.write_policy = WritePolicy::ProposeConfirm;
        assert!(
            store
                .propose(
                    &brain,
                    request(NoteWriteKind::Create, "Elsewhere/No.md", "body"),
                    &options(),
                    &[],
                )
                .unwrap_err()
                .to_string()
                .contains("write directories")
        );

        let mut no_registered_directories = brain.clone();
        no_registered_directories.write_directories.clear();
        assert!(
            store
                .propose(
                    &no_registered_directories,
                    request(NoteWriteKind::Create, "Notes/NoScope.md", "body"),
                    &options(),
                    &[],
                )
                .unwrap_err()
                .to_string()
                .contains("registered write directories")
        );
        assert!(
            store
                .propose(
                    &brain,
                    request(NoteWriteKind::Create, "../outside.md", "body"),
                    &options(),
                    &[],
                )
                .is_err()
        );
        assert!(
            store
                .propose(
                    &brain,
                    request(NoteWriteKind::Create, "Notes/not-markdown.txt", "body"),
                    &options(),
                    &[],
                )
                .is_err()
        );
        assert!(
            store
                .propose(
                    &brain,
                    request(NoteWriteKind::Create, "Notes/Default.md", "body"),
                    &WriteProposalOptions::default(),
                    &[],
                )
                .unwrap_err()
                .to_string()
                .contains("allowed write directories")
        );
    }

    #[test]
    fn rejects_private_and_denied_notes_without_a_matching_grant() {
        let temp = TempDir::new("private");
        temp.write("90_Private/Secret.md", "# private note\n");
        fs::create_dir_all(temp.path.join("Notes")).unwrap();
        fs::create_dir_all(temp.path.join("Denied")).unwrap();
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();

        let private_error = store
            .propose(
                &brain,
                request(NoteWriteKind::Edit, "90_Private/Secret.md", "updated"),
                &options(),
                &[],
            )
            .unwrap_err();
        assert!(private_error.to_string().contains("explicit access grant"));

        let proposal = store
            .propose(
                &brain,
                request(NoteWriteKind::Edit, "90_Private/Secret.md", "updated"),
                &options(),
                &[grant("90_Private/Secret.md")],
            )
            .unwrap();
        store
            .confirm(
                &brain,
                &confirmation(&proposal),
                &options(),
                &[grant("90_Private/Secret.md")],
            )
            .unwrap();

        let mut denied_brain = brain.clone();
        denied_brain.write_directories.push("Denied".to_owned());
        let denied_error = store
            .propose(
                &denied_brain,
                request(NoteWriteKind::Create, "Denied/No.md", "body"),
                &WriteProposalOptions {
                    allowed_directory_prefixes: vec!["Denied".to_owned()],
                    ..options()
                },
                &[grant("Denied/No.md")],
            )
            .unwrap_err();
        assert!(denied_error.to_string().contains("denied"));

        let private_content_error = store
            .propose(
                &brain,
                request(
                    NoteWriteKind::Create,
                    "Notes/Private.md",
                    "#private\nsecret",
                ),
                &options(),
                &[],
            )
            .unwrap_err();
        assert!(
            private_content_error
                .to_string()
                .contains("explicit access grant")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_parent_even_when_it_points_back_inside_a_directory() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new("symlink");
        fs::create_dir_all(temp.path.join("Notes/Real")).unwrap();
        symlink(temp.path.join("Notes/Real"), temp.path.join("Notes/Link")).unwrap();
        let brain = brain(temp.path.clone());
        let mut store = WriteProposalStore::default();

        let error = store
            .propose(
                &brain,
                request(NoteWriteKind::Create, "Notes/Link/Escaped.md", "body"),
                &options(),
                &[],
            )
            .unwrap_err();
        assert!(error.to_string().contains("symlinked parent"));
        assert!(!temp.path.join("Notes/Real/Escaped.md").exists());
    }

    #[test]
    fn preview_diff_is_bounded_and_never_exposes_an_unbounded_line() {
        let before = format!("start\n{}\nend\n", "a".repeat(4_000));
        let after = format!("start\n{}\nend\n", "b".repeat(4_000));
        let diff = bounded_markdown_diff(Some(&before), &after, 256);
        assert!(diff.len() <= 256);
        assert!(diff.contains("truncated") || diff.len() == 256);
        assert!(std::str::from_utf8(diff.as_bytes()).is_ok());
    }
}
