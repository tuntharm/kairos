use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const CHAT_ARCHIVE_VERSION: u32 = 1;
const MAX_STORED_SESSIONS: usize = 100;
const MAX_STORED_MESSAGES_PER_SESSION: usize = 160;

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
    archive.version = CHAT_ARCHIVE_VERSION;
    archive.sessions.truncate(MAX_STORED_SESSIONS);
    for session in &mut archive.sessions {
        session.messages.truncate(MAX_STORED_MESSAGES_PER_SESSION);
    }
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
    archive.sessions.truncate(MAX_STORED_SESSIONS);
    session
}

pub fn append_message(
    session: &mut StoredChatSession,
    role: &str,
    content: String,
    source_ids: Vec<String>,
    attachment_names: Vec<String>,
) -> StoredChatMessage {
    let message = StoredChatMessage {
        id: unique_id("message"),
        role: role.to_owned(),
        content,
        created_at: now_rfc3339(),
        source_ids,
        attachment_names,
    };
    session.messages.push(message.clone());
    if session.messages.len() > MAX_STORED_MESSAGES_PER_SESSION {
        let excess = session.messages.len() - MAX_STORED_MESSAGES_PER_SESSION;
        session.messages.drain(..excess);
    }
    session.updated_at = message.created_at.clone();
    message
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
            "user",
            "How should I plan this week?".to_owned(),
            Vec::new(),
            vec!["draft.md".to_owned()],
        );
        archive.sessions[0] = session;
        persist_chat_archive(&path, &archive).unwrap();
        let loaded = load_chat_archive(&path).unwrap();
        assert_eq!(loaded.sessions.len(), 1);
        assert_eq!(
            loaded.sessions[0].messages[0].attachment_names,
            ["draft.md"]
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
