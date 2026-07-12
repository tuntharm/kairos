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

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadPolicy {
    #[serde(default)]
    pub startup_allow: Vec<String>,
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
        version: 1,
        source_router_path: home.join(".codex/brain_router.md"),
        brains: vec![everyday, phd, datter, border],
    })
}
