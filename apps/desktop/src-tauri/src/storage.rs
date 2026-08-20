use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const CHAT_ARCHIVE_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredSpecialistIdentity {
    pub schema_version: u16,
    pub specialist_id: String,
    pub specialist_name: String,
    pub release_id: String,
    pub release_sha256: String,
    pub evaluation_sha256: String,
    pub evidence_boundary_sha256: String,
}

impl StoredSpecialistIdentity {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.specialist_name.trim().is_empty()
            || self.specialist_name.chars().count() > 100
            || kairos_lab::StableId::parse(&self.specialist_id).is_err()
            || kairos_lab::StableId::parse(&self.release_id).is_err()
            || kairos_lab::Sha256Digest::parse(&self.release_sha256).is_err()
            || kairos_lab::Sha256Digest::parse(&self.evaluation_sha256).is_err()
            || kairos_lab::Sha256Digest::parse(&self.evidence_boundary_sha256).is_err()
        {
            return Err("chat archive contains an invalid specialist identity".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredChatMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub attachment_names: Vec<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub provider_label: Option<String>,
    #[serde(default)]
    pub route_brain_ids: Vec<String>,
    #[serde(default)]
    pub specialist_identity: Option<StoredSpecialistIdentity>,
}

pub struct NewStoredChatMessage {
    pub role: String,
    pub content: String,
    pub source_ids: Vec<String>,
    pub attachment_names: Vec<String>,
    pub provider_id: Option<String>,
    pub provider_label: Option<String>,
    pub route_brain_ids: Vec<String>,
    pub specialist_identity: Option<StoredSpecialistIdentity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredChatSession {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub messages: Vec<StoredChatMessage>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatArchive {
    pub version: u32,
    #[serde(default)]
    pub sessions: Vec<StoredChatSession>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
}

impl Default for ChatArchive {
    fn default() -> Self {
        Self {
            version: CHAT_ARCHIVE_VERSION,
            sessions: Vec::new(),
        }
    }
}

pub fn chat_archive_path() -> Result<PathBuf, String> {
    Ok(kairos_core::application_support_dir()
        .map_err(|error| error.to_string())?
        .join("chats.json"))
}

pub fn load_chat_archive(path: &Path) -> Result<ChatArchive, String> {
    if !path.exists() {
        return Ok(ChatArchive::default());
    }
    let mut archive = serde_json::from_str::<ChatArchive>(
        &fs::read_to_string(path)
            .map_err(|error| format!("could not read chat archive: {error}"))?,
    )
    .map_err(|error| format!("chat archive is unreadable: {error}"))?;
    if archive.version > CHAT_ARCHIVE_VERSION {
        return Err("chat archive was written by a newer Kairos version".to_owned());
    }
    for message in archive
        .sessions
        .iter()
        .flat_map(|session| &session.messages)
    {
        if let Some(identity) = &message.specialist_identity {
            if message.role != "assistant" {
                return Err(
                    "chat archive contains specialist identity on a non-assistant message"
                        .to_owned(),
                );
            }
            identity.validate()?;
        }
    }
    archive.version = CHAT_ARCHIVE_VERSION;
    Ok(archive)
}

pub fn persist_chat_archive(path: &Path, archive: &ChatArchive) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "chat archive has no parent directory".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create chat storage: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        format!(
            "{}\n",
            serde_json::to_string_pretty(archive)
                .map_err(|error| format!("could not encode chat archive: {error}"))?
        ),
    )
    .map_err(|error| format!("could not write chat archive: {error}"))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("could not finalise chat archive: {error}"))
}

pub fn create_session(archive: &mut ChatArchive, initial_message: &str) -> StoredChatSession {
    let timestamp = now_rfc3339();
    let session = StoredChatSession {
        id: unique_id("chat"),
        title: title_for(initial_message),
        created_at: timestamp.clone(),
        updated_at: timestamp,
        messages: Vec::new(),
    };
    archive.sessions.insert(0, session.clone());
    session
}

pub fn append_message(
    session: &mut StoredChatSession,
    input: NewStoredChatMessage,
) -> StoredChatMessage {
    let message = StoredChatMessage {
        id: unique_id("message"),
        role: input.role,
        content: input.content,
        created_at: now_rfc3339(),
        source_ids: input.source_ids,
        attachment_names: input.attachment_names,
        provider_id: input.provider_id,
        provider_label: input.provider_label,
        route_brain_ids: input.route_brain_ids,
        specialist_identity: input.specialist_identity,
    };
    session.messages.push(message.clone());
    session.updated_at = message.created_at.clone();
    message
}

pub fn session_summaries(archive: &ChatArchive, query: Option<&str>) -> Vec<ChatSessionSummary> {
    let query = query.unwrap_or_default().trim().to_lowercase();
    let mut sessions = archive
        .sessions
        .iter()
        .filter(|session| query.is_empty() || session.title.to_lowercase().contains(&query))
        .map(|session| ChatSessionSummary {
            id: session.id.clone(),
            title: session.title.clone(),
            created_at: session.created_at.clone(),
            updated_at: session.updated_at.clone(),
            message_count: session.messages.len(),
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    sessions
}

pub fn rename_session(archive: &mut ChatArchive, id: &str, title: &str) -> Result<(), String> {
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() || title.chars().count() > 100 {
        return Err("Chat titles must contain 1–100 characters.".to_owned());
    }
    let session = archive
        .sessions
        .iter_mut()
        .find(|session| session.id == id)
        .ok_or_else(|| "That chat no longer exists.".to_owned())?;
    session.title = title;
    session.updated_at = now_rfc3339();
    Ok(())
}

pub fn delete_session(archive: &mut ChatArchive, id: &str) -> Result<(), String> {
    let before = archive.sessions.len();
    archive.sessions.retain(|session| session.id != id);
    (archive.sessions.len() != before)
        .then_some(())
        .ok_or_else(|| "That chat no longer exists.".to_owned())
}

pub fn title_session_from_first_message(session: &mut StoredChatSession, message: &str) {
    if session.title == "New conversation" && !message.trim().is_empty() {
        session.title = title_for(message);
    }
}

fn title_for(message: &str) -> String {
    let compact = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut title = compact.chars().take(56).collect::<String>();
    if compact.chars().count() > title.chars().count() {
        title.push('…');
    }
    if title.is_empty() {
        "New conversation".to_owned()
    } else {
        title
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{prefix}-{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_round_trips_and_keeps_the_latest_messages() {
        let directory = std::env::temp_dir().join(unique_id("kairos-storage-test"));
        let path = directory.join("chats.json");
        let mut archive = ChatArchive::default();
        let mut session = create_session(&mut archive, "How should I plan this week?");
        append_message(
            &mut session,
            NewStoredChatMessage {
                role: "assistant".to_owned(),
                content: "How should I plan this week?".to_owned(),
                source_ids: Vec::new(),
                attachment_names: vec!["draft.md".to_owned()],
                provider_id: None,
                provider_label: None,
                route_brain_ids: Vec::new(),
                specialist_identity: Some(StoredSpecialistIdentity {
                    schema_version: 1,
                    specialist_id: "surrogate-experiment-reviewer".to_owned(),
                    specialist_name: "Surrogate Experiment Reviewer".to_owned(),
                    release_id: "release-001".to_owned(),
                    release_sha256: "a".repeat(64),
                    evaluation_sha256: "b".repeat(64),
                    evidence_boundary_sha256: "c".repeat(64),
                }),
            },
        );
        archive.sessions[0] = session;
        persist_chat_archive(&path, &archive).unwrap();
        let loaded = load_chat_archive(&path).unwrap();
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(
            loaded.sessions[0].messages[0].attachment_names,
            ["draft.md"]
        );
        assert_eq!(
            loaded.sessions[0].messages[0]
                .specialist_identity
                .as_ref()
                .map(|identity| identity.release_id.as_str()),
            Some("release-001")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn archive_rejects_fabricated_specialist_identity() {
        let directory = std::env::temp_dir().join(unique_id("kairos-storage-identity-test"));
        let path = directory.join("chats.json");
        let mut archive = ChatArchive::default();
        let mut session = create_session(&mut archive, "Review this experiment");
        append_message(
            &mut session,
            NewStoredChatMessage {
                role: "assistant".to_owned(),
                content: "A fabricated result".to_owned(),
                source_ids: Vec::new(),
                attachment_names: Vec::new(),
                provider_id: None,
                provider_label: None,
                route_brain_ids: Vec::new(),
                specialist_identity: Some(StoredSpecialistIdentity {
                    schema_version: 1,
                    specialist_id: "surrogate-experiment-reviewer".to_owned(),
                    specialist_name: "Surrogate Experiment Reviewer".to_owned(),
                    release_id: "../../fabricated".to_owned(),
                    release_sha256: "not-a-digest".to_owned(),
                    evaluation_sha256: "b".repeat(64),
                    evidence_boundary_sha256: "c".repeat(64),
                }),
            },
        );
        archive.sessions[0] = session;
        persist_chat_archive(&path, &archive).unwrap();
        assert!(load_chat_archive(&path).is_err());
        fs::remove_dir_all(directory).unwrap();
    }
}
