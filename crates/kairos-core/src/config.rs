use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub enabled: bool,
    #[serde(default)]
    pub read_policy: ReadPolicy,
    pub egress_policy: EgressPolicy,
    pub write_policy: WritePolicy,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KairosConfig {
    pub version: u32,
    pub source_router_path: PathBuf,
    #[serde(default)]
    pub local_model: LocalModelSettings,
    pub brains: Vec<BrainRecord>,
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

pub fn write_config(path: impl AsRef<Path>, config: &KairosConfig, overwrite: bool) -> Result<()> {
    let path = path.as_ref();
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
    fs::write(path, format!("{}\n", serde_json::to_string_pretty(config)?))?;
    Ok(())
}

fn default_read_policy() -> ReadPolicy {
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
        enabled: true,
        read_policy: default_read_policy(),
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::ReadOnly,
    };

    let phd = BrainRecord {
        id: "phd".to_owned(),
        name: "PhD Research Brain".to_owned(),
        role: BrainRole::Phd,
        root_path: phd_root,
        router_paths: vec!["00_System/AI Second Brain Manager.md".to_owned()],
        context_paths: vec!["00_System/PhD Current Context.md".to_owned()],
        enabled: true,
        read_policy: default_read_policy(),
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::ReadOnly,
    };

    let datter = BrainRecord {
        id: "datter".to_owned(),
        name: "Datter AI".to_owned(),
        role: BrainRole::Project,
        root_path: datter_root,
        router_paths: vec!["brain/00_System/Datter Brain Manager.md".to_owned()],
        context_paths: vec!["brain/00_System/Project Map.md".to_owned()],
        enabled: false,
        read_policy: ReadPolicy {
            startup_allow: vec!["brain/00_System/**/*.md".to_owned()],
            ..default_read_policy()
        },
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::ReadOnly,
    };

    let border = BrainRecord {
        id: "border-fiber".to_owned(),
        name: "Border Fiber IDS".to_owned(),
        role: BrainRole::Project,
        root_path: border_root,
        router_paths: vec!["PRODUCT.md".to_owned(), "AGENTS.md".to_owned()],
        context_paths: Vec::new(),
        enabled: false,
        read_policy: ReadPolicy {
            explicit_only_patterns: vec!["**".to_owned()],
            deny_patterns: vec!["**".to_owned()],
            ..default_read_policy()
        },
        egress_policy: EgressPolicy::LocalOnly,
        write_policy: WritePolicy::Prohibited,
    };

    Ok(KairosConfig {
        version: 2,
        source_router_path: home.join(".codex/brain_router.md"),
        local_model: LocalModelSettings::default(),
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
}
