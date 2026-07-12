use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::{CoreError, Result};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrainRole {
    Everyday,
    Phd,
    Project,
    Other,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EgressPolicy {
    LocalOnly,
    CloudAllowed,
    RedactRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WritePolicy {
    ReadOnly,
    ProposeConfirm,
    Prohibited,
}

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://localhost:11434";
pub const DEFAULT_LOCAL_MODEL: &str = "qwen3.6:35b-mlx";
pub const FAST_ROUTER_MODEL: &str = "qwen3:8b";
pub const OPTIONAL_LOCAL_MODELS: [&str; 2] = ["gpt-oss:20b", "glm-4.7-flash"];
pub const DEFAULT_CONTEXT_WINDOW_TOKENS: u32 = 32_768;
pub const CURRENT_CONFIG_VERSION: u32 = 3;

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

fn default_summon_shortcut() -> String {
    "Alt+Space".to_owned()
}

fn default_active_provider_id() -> String {
    "ollama-local".to_owned()
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SummonTarget {
    #[default]
    CompactChat,
    LastSurface,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default = "default_summon_shortcut")]
    pub summon_shortcut: String,
    #[serde(default)]
    pub summon_target: SummonTarget,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default = "default_close_to_hide")]
    pub close_to_hide: bool,
}

fn default_close_to_hide() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            summon_shortcut: default_summon_shortcut(),
            summon_target: SummonTarget::CompactChat,
            launch_at_login: false,
            close_to_hide: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Ollama,
    OpenAi,
    Anthropic,
    CodexCli,
    ClaudeCli,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub label: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

impl ProviderConfig {
    pub fn is_external(&self) -> bool {
        !matches!(self.kind, ProviderKind::Ollama)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceSettings {
    #[serde(default = "default_active_provider_id")]
    pub active_provider_id: String,
    #[serde(default = "default_provider_configs")]
    pub providers: Vec<ProviderConfig>,
}

impl Default for InferenceSettings {
    fn default() -> Self {
        Self {
            active_provider_id: default_active_provider_id(),
            providers: default_provider_configs(),
        }
    }
}

impl InferenceSettings {
    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.providers
            .iter()
            .find(|provider| provider.id == self.active_provider_id && provider.enabled)
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|provider| provider.id == id)
    }
}

fn default_provider_configs() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            id: "ollama-local".to_owned(),
            kind: ProviderKind::Ollama,
            label: "Local Ollama".to_owned(),
            model: Some(DEFAULT_LOCAL_MODEL.to_owned()),
            enabled: true,
        },
        ProviderConfig {
            id: "openai-api".to_owned(),
            kind: ProviderKind::OpenAi,
            label: "OpenAI API".to_owned(),
            model: None,
            enabled: false,
        },
        ProviderConfig {
            id: "anthropic-api".to_owned(),
            kind: ProviderKind::Anthropic,
            label: "Anthropic API".to_owned(),
            model: None,
            enabled: false,
        },
        ProviderConfig {
            id: "codex-cli".to_owned(),
            kind: ProviderKind::CodexCli,
            label: "Codex CLI".to_owned(),
            model: None,
            enabled: false,
        },
        ProviderConfig {
            id: "claude-cli".to_owned(),
            kind: ProviderKind::ClaudeCli,
            label: "Claude Code CLI".to_owned(),
            model: None,
            enabled: false,
        },
    ]
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSetupSettings {
    /// `None` means automatic detection. A number is a chosen memory budget,
    /// not a statement about the physical memory installed in a Mac.
    #[serde(default)]
    pub memory_budget_gb: Option<u16>,
}

fn default_ollama_endpoint() -> String {
    DEFAULT_OLLAMA_ENDPOINT.to_owned()
}

fn default_selected_model() -> String {
    DEFAULT_LOCAL_MODEL.to_owned()
}

fn default_context_window_tokens() -> u32 {
    DEFAULT_CONTEXT_WINDOW_TOKENS
}

fn validate_model_identifier(model: &str) -> Result<String> {
    let model = model.trim();
    if model.is_empty()
        || model.len() > 128
        || model
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return Err(CoreError::InvalidPath(
            "local model must be a non-empty Ollama model identifier without whitespace".to_owned(),
        ));
    }
    Ok(model.to_owned())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelSettings {
    #[serde(default = "default_ollama_endpoint")]
    pub ollama_endpoint: String,
    #[serde(default = "default_selected_model")]
    pub selected_model: String,
    #[serde(default = "default_context_window_tokens")]
    pub context_window_tokens: u32,
}

impl Default for LocalModelSettings {
    fn default() -> Self {
        Self {
            ollama_endpoint: default_ollama_endpoint(),
            selected_model: default_selected_model(),
            context_window_tokens: default_context_window_tokens(),
        }
    }
}

impl LocalModelSettings {
    pub fn set_selected_model(&mut self, model: &str) -> Result<()> {
        self.selected_model = validate_model_identifier(model)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        validate_model_identifier(&self.selected_model)?;
        if self.ollama_endpoint.trim_end_matches('/') != DEFAULT_OLLAMA_ENDPOINT {
            return Err(CoreError::InvalidPath(format!(
                "Kairos alpha only permits loopback Ollama at {DEFAULT_OLLAMA_ENDPOINT}"
            )));
        }
        if !(4_096..=131_072).contains(&self.context_window_tokens) {
            return Err(CoreError::InvalidPath(
                "local model context window must be between 4096 and 131072 tokens".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelChoice {
    pub id: String,
    pub label: String,
    pub role: String,
}

/// Curated selector options for the local alpha. A custom saved model stays
/// visible as an explicit choice rather than being replaced automatically.
pub fn local_model_choices(selected_model: &str) -> Vec<LocalModelChoice> {
    let mut choices = vec![
        LocalModelChoice {
            id: DEFAULT_LOCAL_MODEL.to_owned(),
            label: "Qwen 3.6 35B MLX".to_owned(),
            role: "Default local model".to_owned(),
        },
        LocalModelChoice {
            id: FAST_ROUTER_MODEL.to_owned(),
            label: "Qwen 3 8B".to_owned(),
            role: "Fast router / manual fallback".to_owned(),
        },
        LocalModelChoice {
            id: OPTIONAL_LOCAL_MODELS[0].to_owned(),
            label: "GPT-OSS 20B".to_owned(),
            role: "Optional alternative".to_owned(),
        },
        LocalModelChoice {
            id: OPTIONAL_LOCAL_MODELS[1].to_owned(),
            label: "GLM 4.7 Flash".to_owned(),
            role: "Optional alternative".to_owned(),
        },
    ];
    if !selected_model.trim().is_empty()
        && !choices
            .iter()
            .any(|choice| choice.id == selected_model.trim())
    {
        choices.push(LocalModelChoice {
            id: selected_model.trim().to_owned(),
            label: selected_model.trim().to_owned(),
            role: "Custom saved model".to_owned(),
        });
    }
    choices
}

fn default_retrieval_allow() -> Vec<String> {
    vec![
        "01_Daily/**/*.md".to_owned(),
        "02_Projects/**/*.md".to_owned(),
        "04_Areas/**/*.md".to_owned(),
        "05_Knowledge/**/*.md".to_owned(),
        "01_Project/**/*.md".to_owned(),
        "02_Literature/**/*.md".to_owned(),
        "04_Experiment/**/*.md".to_owned(),
        "Daily Note/**/*.md".to_owned(),
    ]
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadPolicy {
    #[serde(default)]
    pub startup_allow: Vec<String>,
    #[serde(default = "default_retrieval_allow")]
    pub retrieval_allow: Vec<String>,
    #[serde(default)]
    pub explicit_only_patterns: Vec<String>,
    #[serde(default)]
    pub deny_patterns: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrainRecord {
    pub id: String,
    pub name: String,
    pub role: BrainRole,
    pub root_path: PathBuf,
    #[serde(default)]
    pub router_paths: Vec<String>,
    #[serde(default)]
    pub context_paths: Vec<String>,
    /// Human-approved vocabulary that helps Kairos route questions without
    /// reading a whole vault. This is metadata, not retrieved note content.
    #[serde(default)]
    pub routing_hints: Vec<String>,
    pub enabled: bool,
    #[serde(default)]
    pub read_policy: ReadPolicy,
    pub egress_policy: EgressPolicy,
    pub write_policy: WritePolicy,
    /// Literal relative directories where confirmed Markdown create/edit
    /// proposals may be issued. Empty is fail-closed.
    #[serde(default)]
    pub write_directories: Vec<String>,
    /// Graph indexing is separately opt-in from text retrieval.
    #[serde(default = "default_graph_enabled")]
    pub graph_enabled: bool,
}

fn default_graph_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KairosConfig {
    pub version: u32,
    pub source_router_path: PathBuf,
    #[serde(default)]
    pub local_model: LocalModelSettings,
    #[serde(default)]
    pub app: AppSettings,
    #[serde(default)]
    pub inference: InferenceSettings,
    #[serde(default)]
    pub local_setup: LocalSetupSettings,
    pub brains: Vec<BrainRecord>,
}

impl KairosConfig {
    pub fn active_provider(&self) -> Option<&ProviderConfig> {
        self.inference.active_provider()
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.inference.provider(id)
    }

    pub fn ensure_current_version(&mut self) {
        let migrating_from_legacy = self.version < CURRENT_CONFIG_VERSION;
        if migrating_from_legacy {
            self.version = CURRENT_CONFIG_VERSION;
        }
        if migrating_from_legacy {
            for brain in &mut self.brains {
                // The legacy profile treats this project as explicit-only and
                // disabled. Do not promote its default serde graph setting
                // into an indexable source during migration.
                if brain.id == "border-fiber" {
                    brain.graph_enabled = false;
                }
            }
        }
        let local_provider = self
            .inference
            .providers
            .iter_mut()
            .find(|provider| matches!(provider.kind, ProviderKind::Ollama));
        match local_provider {
            Some(provider) => provider.model = Some(self.local_model.selected_model.clone()),
            None => self.inference.providers.insert(
                0,
                ProviderConfig {
                    id: default_active_provider_id(),
                    kind: ProviderKind::Ollama,
                    label: "Local Ollama".to_owned(),
                    model: Some(self.local_model.selected_model.clone()),
                    enabled: true,
                },
            ),
        }
        if self.inference.active_provider_id.trim().is_empty() {
            self.inference.active_provider_id = default_active_provider_id();
        }
    }
}

pub fn application_support_dir() -> Result<PathBuf> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| CoreError::InvalidPath("HOME is not available".to_owned()))?;
    Ok(home.join("Library/Application Support/Kairos"))
}

pub fn default_config_path() -> Result<PathBuf> {
    Ok(application_support_dir()?.join("brains.json"))
}

pub fn load_config(path: impl AsRef<Path>) -> Result<KairosConfig> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(CoreError::ConfigMissing(path.to_path_buf()));
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// Read a configuration and, when needed, atomically migrate its v2 shape to
/// v3. The original v2 JSON is retained beside the live settings file before
/// the upgraded file replaces it.
pub fn load_or_migrate_config(path: impl AsRef<Path>) -> Result<KairosConfig> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(CoreError::ConfigMissing(path.to_path_buf()));
    }
    // Keep the exact bytes that were parsed so the backup cannot accidentally
    // preserve a different concurrent version of the registry.
    let original_bytes = fs::read(path)?;
    let mut config = serde_json::from_slice::<KairosConfig>(&original_bytes)?;
    if config.version > CURRENT_CONFIG_VERSION {
        return Err(CoreError::InvalidPath(format!(
            "Kairos settings version {} is newer than this app supports ({CURRENT_CONFIG_VERSION})",
            config.version
        )));
    }
    if config.version < CURRENT_CONFIG_VERSION {
        backup_legacy_config(path, &original_bytes)?;
        config.ensure_current_version();
        if fs::read(path)? != original_bytes {
            return Err(CoreError::InvalidPath(
                "Kairos settings changed while migration was in progress; retry the action"
                    .to_owned(),
            ));
        }
        write_config(path, &config, true)?;
    }
    Ok(config)
}

pub fn migration_backup_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let filename = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("brains");
    path.with_file_name(format!("{filename}.v2.backup.json"))
}

fn backup_legacy_config(path: &Path, contents: &[u8]) -> Result<()> {
    let backup = migration_backup_path(path);
    if backup.exists() {
        return validate_matching_backup(&backup, contents);
    }
    let parent = backup.parent().ok_or_else(|| {
        CoreError::InvalidPath(format!("{} has no parent directory", backup.display()))
    })?;
    fs::create_dir_all(parent)?;
    match atomic_create_bytes(&backup, contents)? {
        true => Ok(()),
        false => validate_matching_backup(&backup, contents),
    }
}

pub fn write_config(path: impl AsRef<Path>, config: &KairosConfig, overwrite: bool) -> Result<()> {
    let path = path.as_ref();
    if config.version != CURRENT_CONFIG_VERSION {
        return Err(CoreError::InvalidPath(format!(
            "refusing to write config version {}; migrate it to {CURRENT_CONFIG_VERSION} first",
            config.version
        )));
    }
    if path.exists() && !overwrite {
        return Err(CoreError::InvalidPath(format!(
            "config already exists at {}; pass overwrite deliberately",
            path.display()
        )));
    }
    let parent = path.parent().ok_or_else(|| {
        CoreError::InvalidPath(format!("{} has no parent directory", path.display()))
    })?;
    fs::create_dir_all(parent)?;
    let contents = format!("{}\n", serde_json::to_string_pretty(config)?);
    if overwrite {
        atomic_replace_bytes(path, contents.as_bytes())
    } else if atomic_create_bytes(path, contents.as_bytes())? {
        Ok(())
    } else {
        Err(CoreError::InvalidPath(format!(
            "config already exists at {}; pass overwrite deliberately",
            path.display()
        )))
    }
}

fn validate_matching_backup(backup: &Path, expected: &[u8]) -> Result<()> {
    let existing = fs::read(backup)?;
    if existing == expected {
        Ok(())
    } else {
        Err(CoreError::InvalidPath(format!(
            "existing migration backup at {} does not match the legacy settings; refusing to replace it",
            backup.display()
        )))
    }
}

fn temporary_file(path: &Path) -> Result<(PathBuf, fs::File)> {
    let parent = path.parent().ok_or_else(|| {
        CoreError::InvalidPath(format!("{} has no parent directory", path.display()))
    })?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            CoreError::InvalidPath(format!("{} has no valid file name", path.display()))
        })?;
    for _ in 0..32 {
        let sequence = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".{name}.{}.{}.tmp", std::process::id(), sequence));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(CoreError::InvalidPath(format!(
        "could not allocate an atomic temporary file beside {}",
        path.display()
    )))
}

fn write_temporary_bytes(path: &Path, contents: &[u8]) -> Result<PathBuf> {
    let (temporary, mut file) = temporary_file(path)?;
    if let Err(error) = file.write_all(contents).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    drop(file);
    Ok(temporary)
}

fn atomic_replace_bytes(path: &Path, contents: &[u8]) -> Result<()> {
    let temporary = write_temporary_bytes(path, contents)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

/// Create a file atomically without replacing an existing destination. The
/// temporary and destination share a directory, so a hard link publishes the
/// fully-written file in one step. The return value reports whether this caller
/// created the destination.
fn atomic_create_bytes(path: &Path, contents: &[u8]) -> Result<bool> {
    let temporary = write_temporary_bytes(path, contents)?;
    let result = match fs::hard_link(&temporary, path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    };
    let _ = fs::remove_file(&temporary);
    result
}

pub fn default_brain_read_policy() -> ReadPolicy {
    ReadPolicy {
        startup_allow: vec!["00_System/**/*.md".to_owned()],
        retrieval_allow: default_retrieval_allow(),
        explicit_only_patterns: vec!["90_Private/**".to_owned()],
        deny_patterns: vec![
            ".git/**".to_owned(),
            ".obsidian/**".to_owned(),
            ".env*".to_owned(),
            "**/.env*".to_owned(),
            "**/*.pem".to_owned(),
            "**/*.key".to_owned(),
            "Attachments/**".to_owned(),
        ],
    }
}

/// Seed a profile for this private alpha. It is never written automatically.
pub fn default_tharm_config() -> Result<KairosConfig> {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| CoreError::InvalidPath("HOME is not available".to_owned()))?;
    let everyday_root = home.join("everyday-life-brain");
    let phd_root = home
        .join("Library/CloudStorage/OneDrive-ImperialCollegeLondon/PhD/PhD - Surrogate Modelling");
    let datter_root = home.join("dev/datter");
    let border_root = home.join("border-fiber-ids");

    let everyday = BrainRecord {
        id: "everyday".to_owned(),
        name: "Everyday Life Brain".to_owned(),
        role: BrainRole::Everyday,
        root_path: everyday_root,
        router_paths: vec!["00_System/AI Brain Manager.md".to_owned()],
        context_paths: vec![
            "00_System/Tharm Model.md".to_owned(),
            "00_System/Current Context.md".to_owned(),
        ],
        routing_hints: vec![
            "life".to_owned(),
            "personal".to_owned(),
            "today".to_owned(),
            "admin".to_owned(),
        ],
        enabled: true,
        read_policy: default_brain_read_policy(),
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::ProposeConfirm,
        write_directories: vec![
            "01_Daily".to_owned(),
            "02_Projects".to_owned(),
            "03_People".to_owned(),
            "04_Areas".to_owned(),
            "05_Knowledge".to_owned(),
            "07_Decisions".to_owned(),
        ],
        graph_enabled: true,
    };

    let phd = BrainRecord {
        id: "phd".to_owned(),
        name: "PhD Research Brain".to_owned(),
        role: BrainRole::Phd,
        root_path: phd_root,
        router_paths: vec!["00_System/AI Second Brain Manager.md".to_owned()],
        context_paths: vec!["00_System/PhD Current Context.md".to_owned()],
        routing_hints: vec![
            "phd".to_owned(),
            "research".to_owned(),
            "abaqus".to_owned(),
            "surrogate".to_owned(),
        ],
        enabled: true,
        read_policy: default_brain_read_policy(),
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::ReadOnly,
        write_directories: Vec::new(),
        graph_enabled: true,
    };

    let datter = BrainRecord {
        id: "datter".to_owned(),
        name: "Datter AI".to_owned(),
        role: BrainRole::Project,
        root_path: datter_root,
        router_paths: vec!["brain/00_System/Datter Brain Manager.md".to_owned()],
        context_paths: vec!["brain/00_System/Project Map.md".to_owned()],
        routing_hints: vec![
            "datter".to_owned(),
            "data usefulness".to_owned(),
            "token waste".to_owned(),
        ],
        enabled: false,
        read_policy: ReadPolicy {
            startup_allow: vec!["brain/00_System/**/*.md".to_owned()],
            ..default_brain_read_policy()
        },
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::ReadOnly,
        write_directories: Vec::new(),
        graph_enabled: true,
    };

    let border = BrainRecord {
        id: "border-fiber".to_owned(),
        name: "Border Fiber IDS".to_owned(),
        role: BrainRole::Project,
        root_path: border_root,
        router_paths: vec!["PRODUCT.md".to_owned(), "AGENTS.md".to_owned()],
        context_paths: Vec::new(),
        routing_hints: vec![
            "border fiber".to_owned(),
            "fiber ids".to_owned(),
            "phi-otdr".to_owned(),
            "das".to_owned(),
        ],
        enabled: false,
        read_policy: ReadPolicy {
            explicit_only_patterns: vec!["**".to_owned()],
            deny_patterns: vec!["**".to_owned()],
            ..default_brain_read_policy()
        },
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::Prohibited,
        write_directories: Vec::new(),
        graph_enabled: false,
    };

    Ok(KairosConfig {
        version: CURRENT_CONFIG_VERSION,
        source_router_path: home.join(".codex/brain_router.md"),
        local_model: LocalModelSettings::default(),
        app: AppSettings::default(),
        inference: InferenceSettings::default(),
        local_setup: LocalSetupSettings::default(),
        brains: vec![everyday, phd, datter, border],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_local_model_uses_32k_and_the_requested_primary_model() {
        let settings = LocalModelSettings::default();
        assert_eq!(settings.ollama_endpoint, DEFAULT_OLLAMA_ENDPOINT);
        assert_eq!(settings.selected_model, DEFAULT_LOCAL_MODEL);
        assert_eq!(settings.context_window_tokens, 32_768);
    }

    #[test]
    fn custom_model_selection_never_coerces_to_a_catalog_model() {
        let mut settings = LocalModelSettings::default();
        settings.set_selected_model("my-model:latest").unwrap();
        assert_eq!(settings.selected_model, "my-model:latest");
        assert!(
            local_model_choices(&settings.selected_model)
                .iter()
                .any(|choice| choice.id == "my-model:latest")
        );
    }

    #[test]
    fn v2_config_migrates_with_a_backup_and_keeps_the_selected_local_model() {
        let directory = std::env::temp_dir().join(format!(
            "kairos-config-migration-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("brains.json");
        let v2 = serde_json::json!({
            "version": 2,
            "sourceRouterPath": "/tmp/router.md",
            "localModel": {
                "ollamaEndpoint": DEFAULT_OLLAMA_ENDPOINT,
                "selectedModel": "qwen3:8b",
                "contextWindowTokens": 32768
            },
            "brains": []
        });
        fs::write(&path, serde_json::to_string_pretty(&v2).unwrap()).unwrap();

        let config = load_or_migrate_config(&path).unwrap();
        assert_eq!(config.version, CURRENT_CONFIG_VERSION);
        assert_eq!(config.local_model.selected_model, "qwen3:8b");
        assert_eq!(
            config
                .active_provider()
                .and_then(|provider| provider.model.as_deref()),
            Some("qwen3:8b")
        );
        assert!(migration_backup_path(&path).exists());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn migration_preserves_existing_write_policy_instead_of_granting_access() {
        let directory = std::env::temp_dir().join(format!(
            "kairos-config-write-policy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("brains.json");
        let mut legacy = default_tharm_config().unwrap();
        legacy.version = 2;
        let everyday = legacy
            .brains
            .iter_mut()
            .find(|brain| brain.id == "everyday")
            .unwrap();
        everyday.write_policy = WritePolicy::ReadOnly;
        everyday.write_directories.clear();
        fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let migrated = load_or_migrate_config(&path).unwrap();
        let everyday = migrated
            .brains
            .iter()
            .find(|brain| brain.id == "everyday")
            .unwrap();
        assert_eq!(everyday.write_policy, WritePolicy::ReadOnly);
        assert!(everyday.write_directories.is_empty());

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn migration_refuses_to_overwrite_a_mismatched_legacy_backup() {
        let directory = std::env::temp_dir().join(format!(
            "kairos-config-backup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("brains.json");
        let contents = serde_json::to_vec(&serde_json::json!({
            "version": 2,
            "sourceRouterPath": "/tmp/router.md",
            "localModel": {
                "ollamaEndpoint": DEFAULT_OLLAMA_ENDPOINT,
                "selectedModel": "qwen3:8b",
                "contextWindowTokens": 32768
            },
            "brains": []
        }))
        .unwrap();
        fs::write(&path, &contents).unwrap();
        fs::write(migration_backup_path(&path), b"different legacy config").unwrap();

        let error = load_or_migrate_config(&path).unwrap_err();
        assert!(error.to_string().contains("does not match"));
        assert_eq!(fs::read(&path).unwrap(), contents);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn newer_config_versions_are_not_rewritten() {
        let directory = std::env::temp_dir().join(format!(
            "kairos-config-future-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("brains.json");
        let mut config = default_tharm_config().unwrap();
        config.version = CURRENT_CONFIG_VERSION + 1;
        let contents = serde_json::to_vec(&config).unwrap();
        fs::write(&path, &contents).unwrap();

        let error = load_or_migrate_config(&path).unwrap_err();
        assert!(error.to_string().contains("newer than this app supports"));
        assert_eq!(fs::read(&path).unwrap(), contents);

        fs::remove_dir_all(directory).unwrap();
    }
}
