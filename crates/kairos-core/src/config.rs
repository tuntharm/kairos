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
pub const FAST_ROUTER_MODEL: &str = "lfm2.5:8b-a1b-q4_K_M";
pub const OPTIONAL_LOCAL_MODELS: [&str; 2] = ["gpt-oss:20b", "glm-4.7-flash:latest"];
pub const DEFAULT_CONTEXT_WINDOW_TOKENS: u32 = 32_768;
pub const CURRENT_CONFIG_VERSION: u32 = 3;
/// The explicit values offered by Kairos's macOS local-model setup. These are
/// planning budgets, not a claim about installed physical RAM and not a
/// resource limit applied to Ollama.
pub const MEMORY_BUDGET_PRESETS_GB: [u16; 7] = [16, 24, 32, 48, 64, 96, 192];

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

fn default_summon_shortcut() -> String {
    "Alt+Space".to_owned()
}

fn default_active_provider_id() -> String {
    "ollama-local".to_owned()
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
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
    /// A normal window is the default. This is deliberately separate from
    /// workspace visibility so users can opt into a floating palette without
    /// making Kairos appear on every macOS Space.
    #[serde(default)]
    pub keep_above_other_windows: bool,
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
            keep_above_other_windows: false,
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

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryBudgetMode {
    #[default]
    Auto,
    Preset,
    Custom,
}

/// Which resource Kairos should use as the primary local-model fit limit.
/// `Auto` always resolves to the detected machine type; the other variants
/// are deliberately planning overrides and never claim that the current
/// machine has that resource.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HardwareProfile {
    #[default]
    Auto,
    AppleUnified,
    NvidiaVram,
    CpuOnly,
}

impl HardwareProfile {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::AppleUnified => "Apple Unified",
            Self::NvidiaVram => "NVIDIA VRAM",
            Self::CpuOnly => "CPU-only",
        }
    }

    pub fn primary_resource_label(&self) -> &'static str {
        match self {
            Self::Auto => "detected primary memory",
            Self::AppleUnified => "Apple unified memory",
            Self::NvidiaVram => "NVIDIA VRAM",
            Self::CpuOnly => "CPU system RAM",
        }
    }
}

/// A successful real local load at the requested resource tier. The key is
/// intentionally exact: catalog tag, effective hardware profile, effective
/// capacity, and context all have to match before a test-required model can
/// graduate out of the advanced catalog.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelVerification {
    pub model: String,
    pub resolved_model: String,
    /// Ollama manifest digest for the exact installed artifact. Empty legacy
    /// records are intentionally not considered verified.
    #[serde(default)]
    pub digest: String,
    /// Exact catalog quantization/profile identity that was verified. Older
    /// records deserialize as empty and intentionally do not promote a model.
    #[serde(default)]
    pub quantization: String,
    pub hardware_profile: HardwareProfile,
    pub effective_capacity_gb: u16,
    pub context_window_tokens: u32,
    pub verified_at_unix_seconds: u64,
}

/// A concrete locally installed Ollama artifact. Verification is deliberately
/// bound to this runtime identity rather than a mutable family name such as
/// `:latest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalModelArtifact {
    pub resolved_model: String,
    pub digest: String,
    pub quantization: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSetupSettings {
    /// `None` means automatic detection. A number is a chosen memory budget,
    /// not a statement about the physical memory installed in a Mac.
    #[serde(default)]
    pub memory_budget_gb: Option<u16>,
    /// Kept separately so a custom value such as `48 GB` remains custom when
    /// it happens to equal one of the preset values. The explicit mode also
    /// lets the WebView restore Auto faithfully after an app restart.
    #[serde(default)]
    pub memory_budget_mode: MemoryBudgetMode,
    /// `Auto` uses the detected machine. A manual profile is a clearly
    /// labelled planning override and uses an explicit chosen budget.
    #[serde(default)]
    pub hardware_profile: HardwareProfile,
    /// Successful real 32K loads. This is local capability metadata only;
    /// recording it never selects a model or changes the context window.
    #[serde(default)]
    pub verified_models: Vec<LocalModelVerification>,
}

impl Default for LocalSetupSettings {
    fn default() -> Self {
        Self {
            memory_budget_gb: None,
            memory_budget_mode: MemoryBudgetMode::Auto,
            hardware_profile: HardwareProfile::Auto,
            verified_models: Vec::new(),
        }
    }
}

impl LocalSetupSettings {
    /// Older v3 alpha builds stored only `memoryBudgetGb`. Infer a mode when
    /// reading such a file so the user's existing choice continues to work;
    /// the next deliberate save writes the explicit mode.
    pub fn resolved_memory_budget_mode(&self) -> MemoryBudgetMode {
        if self.memory_budget_mode == MemoryBudgetMode::Auto && self.memory_budget_gb.is_some() {
            if self
                .memory_budget_gb
                .is_some_and(|budget| MEMORY_BUDGET_PRESETS_GB.contains(&budget))
            {
                MemoryBudgetMode::Preset
            } else {
                MemoryBudgetMode::Custom
            }
        } else {
            self.memory_budget_mode.clone()
        }
    }

    pub fn effective_memory_budget_gb(&self, detected_memory_gb: Option<u16>) -> Option<u16> {
        let configured_cap = match self.resolved_memory_budget_mode() {
            MemoryBudgetMode::Auto => None,
            MemoryBudgetMode::Preset | MemoryBudgetMode::Custom => self.memory_budget_gb,
        };
        match self.hardware_profile {
            // Auto is an actual-machine mode: a preference can only reduce a
            // detected primary resource, never turn 48 GB into a 96 GB fit.
            HardwareProfile::Auto => match (detected_memory_gb, configured_cap) {
                (Some(detected), Some(cap)) => Some(detected.min(cap)),
                (detected, None) => detected,
                (None, _) => None,
            },
            // Manual profiles are explicitly hypothetical planning targets.
            // Their capacity comes from the chosen cap and is never claimed
            // to be the physical machine's memory.
            HardwareProfile::AppleUnified
            | HardwareProfile::NvidiaVram
            | HardwareProfile::CpuOnly => configured_cap,
        }
    }

    pub fn set_memory_budget(
        &mut self,
        mode: MemoryBudgetMode,
        memory_budget_gb: Option<u16>,
    ) -> Result<()> {
        match mode {
            MemoryBudgetMode::Auto => {
                if memory_budget_gb.is_some() {
                    return Err(CoreError::InvalidPath(
                        "Auto memory budget cannot include a manual GB value".to_owned(),
                    ));
                }
            }
            MemoryBudgetMode::Preset => {
                let Some(memory_budget_gb) = memory_budget_gb else {
                    return Err(CoreError::InvalidPath(
                        "Choose one of Kairos's memory-budget presets".to_owned(),
                    ));
                };
                if !MEMORY_BUDGET_PRESETS_GB.contains(&memory_budget_gb) {
                    return Err(CoreError::InvalidPath(format!(
                        "Choose one of Kairos's memory-budget presets: {} GB",
                        MEMORY_BUDGET_PRESETS_GB
                            .iter()
                            .map(u16::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
            }
            MemoryBudgetMode::Custom => {
                let Some(memory_budget_gb) = memory_budget_gb else {
                    return Err(CoreError::InvalidPath(
                        "Enter a custom memory budget between 1 GB and 192 GB".to_owned(),
                    ));
                };
                if !(1..=192).contains(&memory_budget_gb) {
                    return Err(CoreError::InvalidPath(
                        "Choose a custom memory budget between 1 GB and 192 GB".to_owned(),
                    ));
                }
            }
        }
        self.memory_budget_mode = mode;
        self.memory_budget_gb = memory_budget_gb;
        Ok(())
    }

    pub fn set_hardware_profile(&mut self, hardware_profile: HardwareProfile) {
        self.hardware_profile = hardware_profile;
    }

    pub fn matching_verification(
        &self,
        model: &str,
        hardware_profile: &HardwareProfile,
        effective_capacity_gb: Option<u16>,
        context_window_tokens: u32,
        artifact: Option<&LocalModelArtifact>,
    ) -> Option<&LocalModelVerification> {
        let effective_capacity_gb = effective_capacity_gb?;
        let artifact = artifact?;
        if artifact.resolved_model.trim().is_empty()
            || artifact.digest.trim().is_empty()
            || artifact.quantization.trim().is_empty()
        {
            return None;
        }
        let expected_quantization =
            local_model_profile_spec(model).map(|profile| profile.quantization);
        if expected_quantization
            .is_some_and(|expected| !quantization_matches(&artifact.quantization, expected))
        {
            return None;
        }
        self.verified_models.iter().find(|verification| {
            // A legacy record that only carried an untagged name, or one
            // without a digest/manifest quantization, must not promote a
            // mutable new pull into the default list.
            verification.resolved_model == artifact.resolved_model
                && verification.digest == artifact.digest
                && quantization_matches(&verification.quantization, &artifact.quantization)
                && expected_quantization.is_none_or(|expected| {
                    quantization_matches(&verification.quantization, expected)
                })
                && verification.hardware_profile == *hardware_profile
                && verification.effective_capacity_gb == effective_capacity_gb
                && verification.context_window_tokens == context_window_tokens
        })
    }

    pub fn record_verification(&mut self, verification: LocalModelVerification) {
        self.verified_models.retain(|existing| {
            !(model_ids_match(&existing.model, &verification.model)
                && existing.resolved_model == verification.resolved_model
                && existing.digest == verification.digest
                && existing.quantization == verification.quantization
                && existing.hardware_profile == verification.hardware_profile
                && existing.effective_capacity_gb == verification.effective_capacity_gb
                && existing.context_window_tokens == verification.context_window_tokens)
        });
        self.verified_models.push(verification);
        // This is small, human-readable settings metadata rather than an
        // unbounded history. Keep the newest entries if tests are repeated.
        if self.verified_models.len() > 64 {
            self.verified_models
                .sort_by_key(|item| item.verified_at_unix_seconds);
            let to_remove = self.verified_models.len() - 64;
            self.verified_models.drain(0..to_remove);
        }
    }
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

/// Fixed metadata for exact Ollama tags. Package size is intentionally
/// display-only: the hardware fit table below reserves separate 32K headroom
/// and is never inferred from download size.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelProfile {
    pub id: String,
    pub label: String,
    pub role: String,
    pub quantization: String,
    pub capabilities: Vec<String>,
    pub package_size_bytes: Option<u64>,
    pub minimum_memory_gb: u16,
    pub recommended_memory_gb: u16,
    pub maximum_context_tokens: Option<u32>,
    pub release_date: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
    pub source_url: Option<String>,
    pub catalog_digest: Option<String>,
    pub reasoning_mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalModelFit {
    Recommended,
    Tight,
    NotRecommended,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelFitAssessment {
    pub fit: LocalModelFit,
    pub budget_gb: Option<u16>,
    pub minimum_memory_gb: Option<u16>,
    pub recommended_memory_gb: Option<u16>,
    pub context_window_tokens: u32,
    pub maximum_context_tokens: Option<u32>,
    pub context_compatible: Option<bool>,
    pub requires_test: bool,
    pub verified_at_32k: bool,
    pub hardware_profile: HardwareProfile,
    pub message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelRecommendation {
    pub profile: LocalModelProfile,
    pub fit: LocalModelFitAssessment,
    pub default_recommended: bool,
    pub advanced_only: bool,
    pub why_recommended: String,
    pub recommendation_rank: Option<u8>,
}

#[derive(Clone, Copy)]
struct MemoryRequirement {
    minimum_gb: u16,
    comfortable_gb: u16,
}

#[derive(Clone, Copy)]
struct HardwareFitTable {
    apple_unified: Option<MemoryRequirement>,
    nvidia_vram: Option<MemoryRequirement>,
    cpu_only: Option<MemoryRequirement>,
}

#[derive(Clone, Copy)]
struct LocalModelProfileSpec {
    id: &'static str,
    aliases: &'static [&'static str],
    label: &'static str,
    role: &'static str,
    quantization: &'static str,
    capabilities: &'static [&'static str],
    why_recommended: &'static str,
    package_size_bytes: u64,
    fits: HardwareFitTable,
    maximum_context_tokens: u32,
    requires_verification: bool,
}

const LOCAL_MODEL_PROFILE_SPECS: [LocalModelProfileSpec; 16] = [
    LocalModelProfileSpec {
        id: FAST_ROUTER_MODEL,
        aliases: &[],
        label: "LFM2.5 8B A1B",
        role: "Fast everyday chat and routing",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Reasoning", "Routing", "Tools"],
        why_recommended: "A current on-device assistant with low active compute for fast routing, chained tool calls, and private everyday chat.",
        package_size_bytes: 5_200_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 12,
                comfortable_gb: 16,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 12,
                comfortable_gb: 16,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 20,
                comfortable_gb: 24,
            }),
        },
        maximum_context_tokens: 125_000,
        requires_verification: true,
    },
    LocalModelProfileSpec {
        id: "qwen3:8b",
        aliases: &[],
        label: "Qwen 3 8B",
        role: "Legacy fast router (Advanced)",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Routing", "Summaries"],
        why_recommended: "Existing Qwen3 installation retained for an explicit local A/B comparison with LFM2.5.",
        package_size_bytes: 5_200_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 12,
                comfortable_gb: 16,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 12,
                comfortable_gb: 16,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 20,
                comfortable_gb: 24,
            }),
        },
        maximum_context_tokens: 40_000,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "gemma4:12b-mlx",
        aliases: &[],
        label: "Gemma 4 12B MLX",
        role: "Balanced multimodal assistant",
        quantization: "MLX",
        capabilities: &["Text", "Vision", "Audio", "Documents", "Coding", "Tools"],
        why_recommended: "A balanced multimodal laptop model for documents, vision, audio, coding, and tool-aware work.",
        package_size_bytes: 7_700_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 20,
                comfortable_gb: 24,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 16,
                comfortable_gb: 24,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "gemma3:12b",
        aliases: &[],
        label: "Gemma 3 12B",
        role: "Balanced vision and document assistant",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Vision", "Documents"],
        why_recommended: "A compact multimodal choice for documents and image-aware work.",
        package_size_bytes: 8_100_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 20,
                comfortable_gb: 24,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 14,
                comfortable_gb: 16,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "qwen3:14b",
        aliases: &[],
        label: "Qwen 3 14B",
        role: "Balanced general assistant",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Reasoning"],
        why_recommended: "A stronger everyday text model without moving into the large-model tier.",
        package_size_bytes: 9_300_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 20,
                comfortable_gb: 24,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 14,
                comfortable_gb: 16,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
        },
        maximum_context_tokens: 40_000,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: OPTIONAL_LOCAL_MODELS[0],
        aliases: &[],
        label: "GPT-OSS 20B",
        role: "Strong reasoning and structured work",
        quantization: "MXFP4",
        capabilities: &["Text", "Reasoning", "Tools", "Structured output"],
        why_recommended: "The first comfortably fitting reasoning and tools step above everyday models.",
        package_size_bytes: 14_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 20,
                comfortable_gb: 24,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "qwen3.6:27b",
        aliases: &[],
        label: "Qwen 3.6 27B",
        role: "Higher-quality general and vision work",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Vision", "Coding"],
        why_recommended: "A stronger all-round Apple or GPU option when the 32K plan has real headroom.",
        package_size_bytes: 17_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 52,
                comfortable_gb: 64,
            }),
        },
        maximum_context_tokens: 262_144,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "mistral-small3.2:24b",
        aliases: &[],
        label: "Mistral Small 3.2 24B",
        role: "Tools, vision, and document workflows",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Vision", "Tools", "Documents"],
        why_recommended: "A specialist for robust tool calls, documents, and image-aware workflows.",
        package_size_bytes: 15_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 24,
                comfortable_gb: 32,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 44,
                comfortable_gb: 56,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "gemma3:27b",
        aliases: &[],
        label: "Gemma 3 27B",
        role: "High-quality vision and documents",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Vision", "Documents"],
        why_recommended: "The stronger document and vision choice once the 32K plan has comfortable headroom.",
        package_size_bytes: 17_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 52,
                comfortable_gb: 64,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "qwen3:30b",
        aliases: &[],
        label: "Qwen 3 30B",
        role: "Research synthesis and reasoning",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Reasoning", "Research"],
        why_recommended: "A larger reasoning option for research synthesis when it comfortably fits at 32K.",
        package_size_bytes: 19_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 28,
                comfortable_gb: 32,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 56,
                comfortable_gb: 64,
            }),
        },
        maximum_context_tokens: 262_144,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "qwen3:32b",
        aliases: &[],
        label: "Qwen 3 32B",
        role: "Higher-quality dense reasoning",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Reasoning"],
        why_recommended: "The strongest dense Qwen reasoning model in the standard 32K shortlist tier.",
        package_size_bytes: 20_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 30,
                comfortable_gb: 32,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 56,
                comfortable_gb: 64,
            }),
        },
        maximum_context_tokens: 40_000,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: DEFAULT_LOCAL_MODEL,
        aliases: &[],
        label: "Qwen 3.6 35B MLX",
        role: "Strong Apple-Silicon reasoning and coding",
        quantization: "NVFP4",
        capabilities: &["Text", "Vision", "Coding", "Reasoning"],
        why_recommended: "The preferred strong Apple-Silicon option once 48 GB unified memory comfortably covers a 32K chat.",
        package_size_bytes: 22_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: None,
            cpu_only: None,
        },
        maximum_context_tokens: 262_144,
        requires_verification: false,
    },
    LocalModelProfileSpec {
        id: "glm-4.7-flash:latest",
        aliases: &["glm-4.7-flash"],
        label: "GLM 4.7 Flash",
        role: "Coding and local agent specialist",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Coding", "Tools", "Agents"],
        why_recommended: "A strong local coding and agent specialist after a real 32K test on a compatible Ollama build.",
        package_size_bytes: 19_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 40,
                comfortable_gb: 48,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 30,
                comfortable_gb: 32,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 56,
                comfortable_gb: 64,
            }),
        },
        maximum_context_tokens: 198_000,
        requires_verification: true,
    },
    LocalModelProfileSpec {
        id: "llama3.3:70b",
        aliases: &[],
        label: "Llama 3.3 70B",
        role: "Heavy general-purpose assistant",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Reasoning", "Tools"],
        why_recommended: "A large multilingual general model, kept advanced until this exact 32K configuration passes a local test.",
        package_size_bytes: 43_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 80,
                comfortable_gb: 96,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 48,
                comfortable_gb: 64,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 112,
                comfortable_gb: 128,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: true,
    },
    LocalModelProfileSpec {
        id: "gpt-oss:120b",
        aliases: &[],
        label: "GPT-OSS 120B",
        role: "High-end reasoning and tools",
        quantization: "MXFP4",
        capabilities: &["Text", "Reasoning", "Tools", "Structured output"],
        why_recommended: "High-end reasoning, kept advanced until the exact 32K hardware plan is verified locally.",
        package_size_bytes: 65_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 112,
                comfortable_gb: 128,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 80,
                comfortable_gb: 96,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 144,
                comfortable_gb: 160,
            }),
        },
        maximum_context_tokens: 131_072,
        requires_verification: true,
    },
    LocalModelProfileSpec {
        id: "qwen3:235b",
        aliases: &[],
        label: "Qwen 3 235B",
        role: "Workstation-scale reasoning",
        quantization: "Q4_K_M",
        capabilities: &["Text", "Reasoning", "Research"],
        why_recommended: "Workstation-scale reasoning, advanced until a real 32K test confirms this exact configuration.",
        package_size_bytes: 142_000_000_000,
        fits: HardwareFitTable {
            apple_unified: Some(MemoryRequirement {
                minimum_gb: 176,
                comfortable_gb: 192,
            }),
            nvidia_vram: Some(MemoryRequirement {
                minimum_gb: 160,
                comfortable_gb: 192,
            }),
            cpu_only: Some(MemoryRequirement {
                minimum_gb: 224,
                comfortable_gb: 256,
            }),
        },
        maximum_context_tokens: 262_144,
        requires_verification: true,
    },
];

fn normalized_model_id(model: &str) -> &str {
    model.trim().trim_end_matches(":latest")
}

fn model_ids_match(left: &str, right: &str) -> bool {
    normalized_model_id(left) == normalized_model_id(right)
}

fn quantization_matches(left: &str, right: &str) -> bool {
    left.trim().eq_ignore_ascii_case(right.trim())
}

fn local_model_profile_spec(model: &str) -> Option<LocalModelProfileSpec> {
    LOCAL_MODEL_PROFILE_SPECS.iter().copied().find(|profile| {
        model_ids_match(profile.id, model)
            || profile
                .aliases
                .iter()
                .any(|alias| model_ids_match(alias, model))
    })
}

fn requirement_for(
    profile: LocalModelProfileSpec,
    hardware: HardwareProfile,
) -> Option<MemoryRequirement> {
    match hardware {
        HardwareProfile::AppleUnified => profile.fits.apple_unified,
        HardwareProfile::NvidiaVram => profile.fits.nvidia_vram,
        HardwareProfile::CpuOnly => profile.fits.cpu_only,
        HardwareProfile::Auto => None,
    }
}

fn profile_from_spec(profile: LocalModelProfileSpec) -> LocalModelProfile {
    let apple_requirement = profile
        .fits
        .apple_unified
        .or(profile.fits.nvidia_vram)
        .or(profile.fits.cpu_only);
    LocalModelProfile {
        id: profile.id.to_owned(),
        label: profile.label.to_owned(),
        role: profile.role.to_owned(),
        quantization: profile.quantization.to_owned(),
        capabilities: profile
            .capabilities
            .iter()
            .map(|capability| (*capability).to_owned())
            .collect(),
        package_size_bytes: Some(profile.package_size_bytes),
        minimum_memory_gb: apple_requirement
            .map(|fit| fit.minimum_gb)
            .unwrap_or_default(),
        recommended_memory_gb: apple_requirement
            .map(|fit| fit.comfortable_gb)
            .unwrap_or_default(),
        maximum_context_tokens: Some(profile.maximum_context_tokens),
        release_date: model_release_date(profile.id).map(str::to_owned),
        license: model_license(profile.id).map(str::to_owned),
        license_url: model_license_url(profile.id).map(str::to_owned),
        source_url: model_source_url(profile.id).map(str::to_owned),
        catalog_digest: model_catalog_digest(profile.id).map(str::to_owned),
        reasoning_mode: model_reasoning_mode(profile.id).to_owned(),
    }
}

fn model_release_date(model: &str) -> Option<&'static str> {
    match normalized_model_id(model) {
        FAST_ROUTER_MODEL => Some("2026-05-28"),
        "gemma4:12b-mlx" => Some("2026-06-03"),
        DEFAULT_LOCAL_MODEL => None,
        _ => None,
    }
}

fn model_license(model: &str) -> Option<&'static str> {
    match normalized_model_id(model) {
        FAST_ROUTER_MODEL => Some("LFM Open License 1.0"),
        "gemma4:12b-mlx" => Some("Gemma Terms of Use"),
        DEFAULT_LOCAL_MODEL => Some("Ollama artifact terms; verify upstream model licence"),
        _ => None,
    }
}

fn model_license_url(model: &str) -> Option<&'static str> {
    match normalized_model_id(model) {
        FAST_ROUTER_MODEL => Some("https://www.liquid.ai/lfm-license"),
        "gemma4:12b-mlx" => Some("https://ai.google.dev/gemma/terms"),
        _ => None,
    }
}

fn model_source_url(model: &str) -> Option<&'static str> {
    match normalized_model_id(model) {
        FAST_ROUTER_MODEL => Some("https://ollama.com/library/lfm2.5/tags"),
        "gemma4:12b-mlx" => Some("https://ollama.com/library/gemma4:12b-mlx"),
        DEFAULT_LOCAL_MODEL => Some("https://ollama.com/library/qwen3.6:35b-mlx"),
        _ => None,
    }
}

fn model_catalog_digest(model: &str) -> Option<&'static str> {
    match normalized_model_id(model) {
        // Digest is pinned for the current Ollama LFM artifact; other entries
        // remain visibly unverified until the local catalog resolves them.
        FAST_ROUTER_MODEL => Some("9cf756159fc2"),
        _ => None,
    }
}

fn model_reasoning_mode(model: &str) -> &'static str {
    match normalized_model_id(model) {
        FAST_ROUTER_MODEL => "required",
        "gemma4:12b-mlx" => "optional",
        DEFAULT_LOCAL_MODEL => "optional",
        _ => "none",
    }
}

pub fn local_model_profiles() -> Vec<LocalModelProfile> {
    LOCAL_MODEL_PROFILE_SPECS
        .iter()
        .copied()
        .map(profile_from_spec)
        .collect()
}

pub fn local_model_profile(model: &str) -> Option<LocalModelProfile> {
    local_model_profile_spec(model).map(profile_from_spec)
}

pub fn assess_local_model_fit_for_hardware(
    model: &str,
    hardware_profile: HardwareProfile,
    primary_capacity_gb: Option<u16>,
    context_window_tokens: u32,
    verified_at_32k: bool,
) -> LocalModelFitAssessment {
    let profile_label = hardware_profile.label();
    let Some(profile) = local_model_profile_spec(model) else {
        return LocalModelFitAssessment {
            fit: LocalModelFit::Unknown,
            budget_gb: primary_capacity_gb,
            minimum_memory_gb: None,
            recommended_memory_gb: None,
            context_window_tokens,
            maximum_context_tokens: None,
            context_compatible: None,
            requires_test: true,
            verified_at_32k: false,
            hardware_profile,
            message: format!(
                "Kairos has no catalog fit estimate for `{}` on {profile_label}. It will keep the current model and {}K context unchanged.",
                model.trim(),
                context_window_tokens / 1024
            ),
        };
    };
    let Some(requirement) = requirement_for(profile, hardware_profile) else {
        return LocalModelFitAssessment {
            fit: LocalModelFit::NotRecommended,
            budget_gb: primary_capacity_gb,
            minimum_memory_gb: None,
            recommended_memory_gb: None,
            context_window_tokens,
            maximum_context_tokens: Some(profile.maximum_context_tokens),
            context_compatible: Some(context_window_tokens <= profile.maximum_context_tokens),
            requires_test: false,
            verified_at_32k: false,
            hardware_profile,
            message: format!(
                "Current model is not recommended for {profile_label}: `{}` is not a supported Ollama variant for that hardware profile. Kairos will not switch models automatically.",
                profile.id
            ),
        };
    };
    if context_window_tokens > profile.maximum_context_tokens {
        return LocalModelFitAssessment {
            fit: LocalModelFit::NotRecommended,
            budget_gb: primary_capacity_gb,
            minimum_memory_gb: Some(requirement.minimum_gb),
            recommended_memory_gb: Some(requirement.comfortable_gb),
            context_window_tokens,
            maximum_context_tokens: Some(profile.maximum_context_tokens),
            context_compatible: Some(false),
            requires_test: false,
            verified_at_32k: false,
            hardware_profile,
            message: format!(
                "Current model is not recommended for {profile_label} at {}K context: {} is catalogued only through {}K. Kairos will not lower context or switch models automatically.",
                context_window_tokens / 1024,
                profile.label,
                profile.maximum_context_tokens / 1024
            ),
        };
    }
    let Some(capacity_gb) = primary_capacity_gb else {
        return LocalModelFitAssessment {
            fit: LocalModelFit::Unknown,
            budget_gb: None,
            minimum_memory_gb: Some(requirement.minimum_gb),
            recommended_memory_gb: Some(requirement.comfortable_gb),
            context_window_tokens,
            maximum_context_tokens: Some(profile.maximum_context_tokens),
            context_compatible: Some(true),
            requires_test: profile.requires_verification,
            verified_at_32k: false,
            hardware_profile,
            message: format!(
                "Choose a {profile_label} capacity before Kairos can assess {} for {}K context. The current model and context remain unchanged.",
                profile.label,
                context_window_tokens / 1024
            ),
        };
    };
    if capacity_gb < requirement.minimum_gb {
        return LocalModelFitAssessment {
            fit: LocalModelFit::NotRecommended,
            budget_gb: Some(capacity_gb),
            minimum_memory_gb: Some(requirement.minimum_gb),
            recommended_memory_gb: Some(requirement.comfortable_gb),
            context_window_tokens,
            maximum_context_tokens: Some(profile.maximum_context_tokens),
            context_compatible: Some(true),
            requires_test: false,
            verified_at_32k: false,
            hardware_profile,
            message: format!(
                "Current model is not recommended for {profile_label} {capacity_gb} GB at {}K context. {} needs at least {} GB of that primary resource; Kairos will not switch models automatically.",
                context_window_tokens / 1024,
                profile.label,
                requirement.minimum_gb
            ),
        };
    }
    if capacity_gb < requirement.comfortable_gb {
        return LocalModelFitAssessment {
            fit: LocalModelFit::Tight,
            budget_gb: Some(capacity_gb),
            minimum_memory_gb: Some(requirement.minimum_gb),
            recommended_memory_gb: Some(requirement.comfortable_gb),
            context_window_tokens,
            maximum_context_tokens: Some(profile.maximum_context_tokens),
            context_compatible: Some(true),
            requires_test: true,
            verified_at_32k: false,
            hardware_profile,
            message: format!(
                "Current model is tight for {profile_label} {capacity_gb} GB at {}K context. {} GB is Kairos's conservative comfortable tier for {}; keep it in Advanced and test explicitly if you want to try it.",
                context_window_tokens / 1024,
                requirement.comfortable_gb,
                profile.label
            ),
        };
    }
    let requires_test = profile.requires_verification && !verified_at_32k;
    LocalModelFitAssessment {
        fit: LocalModelFit::Recommended,
        budget_gb: Some(capacity_gb),
        minimum_memory_gb: Some(requirement.minimum_gb),
        recommended_memory_gb: Some(requirement.comfortable_gb),
        context_window_tokens,
        maximum_context_tokens: Some(profile.maximum_context_tokens),
        context_compatible: Some(true),
        requires_test,
        verified_at_32k,
        hardware_profile,
        message: if requires_test {
            format!(
                "{} fits the {profile_label} {capacity_gb} GB plan at {}K on paper, but stays Advanced until this exact configuration passes a real local 32K test.",
                profile.label,
                context_window_tokens / 1024
            )
        } else if verified_at_32k {
            format!(
                "Verified locally for {profile_label} {capacity_gb} GB at {}K context. Kairos has not changed the selected model or context.",
                context_window_tokens / 1024
            )
        } else {
            format!(
                "Recommended for {profile_label} {capacity_gb} GB at {}K context with conservative headroom.",
                context_window_tokens / 1024
            )
        },
    }
}

/// Legacy callers retain Apple-unified semantics. Native setup uses the
/// hardware-aware function above so NVIDIA plans never borrow system RAM.
pub fn assess_local_model_fit(
    model: &str,
    memory_budget_gb: Option<u16>,
    context_window_tokens: u32,
) -> LocalModelFitAssessment {
    assess_local_model_fit_for_hardware(
        model,
        HardwareProfile::AppleUnified,
        memory_budget_gb,
        context_window_tokens,
        false,
    )
}

pub fn local_model_recommendations(
    hardware_profile: HardwareProfile,
    primary_capacity_gb: Option<u16>,
    context_window_tokens: u32,
    setup: &LocalSetupSettings,
    installed_artifacts: &[LocalModelArtifact],
) -> Vec<LocalModelRecommendation> {
    let mut recommendations = LOCAL_MODEL_PROFILE_SPECS
        .iter()
        .copied()
        .map(|spec| {
            let artifact = installed_artifacts
                .iter()
                .find(|artifact| artifact.resolved_model == spec.id);
            let verified = setup
                .matching_verification(
                    spec.id,
                    &hardware_profile,
                    primary_capacity_gb,
                    context_window_tokens,
                    artifact,
                )
                .is_some();
            let fit = assess_local_model_fit_for_hardware(
                spec.id,
                hardware_profile,
                primary_capacity_gb,
                context_window_tokens,
                verified,
            );
            LocalModelRecommendation {
                profile: profile_from_spec(spec),
                fit,
                default_recommended: false,
                advanced_only: true,
                why_recommended: spec.why_recommended.to_owned(),
                recommendation_rank: None,
            }
        })
        .collect::<Vec<_>>();

    // The Manager shortlist is deliberately small and stable. A model can be
    // shown here while it still needs an explicit real-runtime test (LFM and
    // the premium model), but only if the conservative fit assessment says it
    // is comfortable for the selected hardware and 32K target.
    let preferred_ids = [FAST_ROUTER_MODEL, "gemma4:12b-mlx", DEFAULT_LOCAL_MODEL];
    for (index, model) in preferred_ids.into_iter().enumerate() {
        if let Some(recommendation) = recommendations.iter_mut().find(|recommendation| {
            recommendation.profile.id == model
                && recommendation.fit.fit == LocalModelFit::Recommended
                && (model == FAST_ROUTER_MODEL
                    || model == DEFAULT_LOCAL_MODEL
                    || !recommendation.fit.requires_test)
        }) {
            recommendation.default_recommended = true;
            recommendation.advanced_only = false;
            recommendation.recommendation_rank = Some(index as u8 + 1);
        }
    }
    recommendations
}

/// Curated selector options for the local alpha. A custom saved model stays
/// visible as an explicit choice rather than being replaced automatically.
pub fn local_model_choices(selected_model: &str) -> Vec<LocalModelChoice> {
    let mut choices = local_model_profiles()
        .into_iter()
        .map(|profile| LocalModelChoice {
            id: profile.id,
            label: profile.label,
            role: profile.role,
        })
        .collect::<Vec<_>>();
    if !selected_model.trim().is_empty()
        && !choices
            .iter()
            .any(|choice| model_ids_match(&choice.id, selected_model))
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
    /// Graph scope is metadata-only and intentionally independent from chat
    /// retrieval. Older registries safely default to all non-protected notes.
    #[serde(default = "default_graph_include_patterns")]
    pub graph_include_patterns: Vec<String>,
}

fn default_graph_enabled() -> bool {
    true
}

fn default_graph_include_patterns() -> Vec<String> {
    vec!["**/*.md".to_owned()]
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

/// Create an empty local registry. A new installation never guesses folders,
/// imports notes, or includes a developer's paths; a person adds each brain
/// through the native picker and explicitly approves its policy.
pub fn default_user_config() -> Result<KairosConfig> {
    Ok(KairosConfig {
        version: CURRENT_CONFIG_VERSION,
        source_router_path: PathBuf::new(),
        local_model: LocalModelSettings::default(),
        app: AppSettings::default(),
        inference: InferenceSettings::default(),
        local_setup: LocalSetupSettings::default(),
        brains: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_brain(id: &str) -> BrainRecord {
        BrainRecord {
            id: id.to_owned(),
            name: "Test brain".to_owned(),
            role: BrainRole::Other,
            root_path: std::env::temp_dir(),
            router_paths: Vec::new(),
            context_paths: Vec::new(),
            routing_hints: Vec::new(),
            enabled: true,
            read_policy: default_brain_read_policy(),
            egress_policy: EgressPolicy::LocalOnly,
            write_policy: WritePolicy::ProposeConfirm,
            write_directories: vec!["Notes".to_owned()],
            graph_enabled: true,
            graph_include_patterns: vec!["**/*.md".to_owned()],
        }
    }

    #[test]
    fn default_local_model_uses_32k_and_the_requested_primary_model() {
        let settings = LocalModelSettings::default();
        assert_eq!(settings.ollama_endpoint, DEFAULT_OLLAMA_ENDPOINT);
        assert_eq!(settings.selected_model, DEFAULT_LOCAL_MODEL);
        assert_eq!(settings.context_window_tokens, 32_768);
    }

    #[test]
    fn default_user_config_is_empty_and_does_not_guess_any_personal_paths() {
        let config = default_user_config().unwrap();
        assert!(config.brains.is_empty());
        assert!(config.source_router_path.as_os_str().is_empty());
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
    fn memory_budget_round_trips_auto_preset_and_custom_without_touching_the_model() {
        let local_model = LocalModelSettings::default();
        let mut setup = LocalSetupSettings::default();

        setup
            .set_memory_budget(MemoryBudgetMode::Auto, None)
            .unwrap();
        assert_eq!(setup.resolved_memory_budget_mode(), MemoryBudgetMode::Auto);
        assert_eq!(setup.effective_memory_budget_gb(Some(48)), Some(48));

        setup
            .set_memory_budget(MemoryBudgetMode::Preset, Some(32))
            .unwrap();
        assert_eq!(
            setup.resolved_memory_budget_mode(),
            MemoryBudgetMode::Preset
        );
        assert_eq!(setup.memory_budget_gb, Some(32));
        assert_eq!(setup.effective_memory_budget_gb(Some(48)), Some(32));

        setup
            .set_memory_budget(MemoryBudgetMode::Preset, Some(96))
            .unwrap();
        assert_eq!(
            setup.effective_memory_budget_gb(Some(48)),
            Some(48),
            "an Auto profile may cap detected hardware but may not inflate it"
        );
        setup.set_hardware_profile(HardwareProfile::NvidiaVram);
        assert_eq!(
            setup.effective_memory_budget_gb(Some(48)),
            Some(96),
            "a manual profile is an explicitly hypothetical planning target"
        );
        setup.set_hardware_profile(HardwareProfile::Auto);

        setup
            .set_memory_budget(MemoryBudgetMode::Custom, Some(37))
            .unwrap();
        assert_eq!(
            setup.resolved_memory_budget_mode(),
            MemoryBudgetMode::Custom
        );
        assert_eq!(setup.memory_budget_gb, Some(37));
        assert_eq!(
            serde_json::to_value(&setup).unwrap()["memoryBudgetMode"],
            "custom"
        );
        assert_eq!(local_model.selected_model, DEFAULT_LOCAL_MODEL);
        assert_eq!(
            local_model.context_window_tokens,
            DEFAULT_CONTEXT_WINDOW_TOKENS
        );

        assert!(
            setup
                .set_memory_budget(MemoryBudgetMode::Preset, Some(37))
                .is_err()
        );
        assert!(
            setup
                .set_memory_budget(MemoryBudgetMode::Auto, Some(48))
                .is_err()
        );
    }

    #[test]
    fn legacy_v3_memory_value_is_inferred_without_losing_the_user_choice() {
        let legacy_preset = LocalSetupSettings {
            memory_budget_gb: Some(32),
            memory_budget_mode: MemoryBudgetMode::Auto,
            ..LocalSetupSettings::default()
        };
        assert_eq!(
            legacy_preset.resolved_memory_budget_mode(),
            MemoryBudgetMode::Preset
        );
        assert_eq!(legacy_preset.effective_memory_budget_gb(Some(48)), Some(32));

        let legacy_custom = LocalSetupSettings {
            memory_budget_gb: Some(37),
            memory_budget_mode: MemoryBudgetMode::Auto,
            ..LocalSetupSettings::default()
        };
        assert_eq!(
            legacy_custom.resolved_memory_budget_mode(),
            MemoryBudgetMode::Custom
        );
        assert_eq!(legacy_custom.effective_memory_budget_gb(Some(48)), Some(37));
    }

    #[test]
    fn catalog_fit_is_hardware_aware_and_never_implies_a_silent_change() {
        let fast = assess_local_model_fit_for_hardware(
            FAST_ROUTER_MODEL,
            HardwareProfile::AppleUnified,
            Some(16),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            false,
        );
        assert_eq!(fast.fit, LocalModelFit::Recommended);
        assert!(fast.requires_test);
        let fast_json = serde_json::to_value(&fast).unwrap();
        assert_eq!(fast_json["fit"], "recommended");
        assert_eq!(fast_json["minimumMemoryGb"], 12);
        assert_eq!(fast_json["contextWindowTokens"], 32_768);
        assert_eq!(fast_json["requiresTest"], true);

        let default_unsupported = assess_local_model_fit_for_hardware(
            DEFAULT_LOCAL_MODEL,
            HardwareProfile::AppleUnified,
            Some(32),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            false,
        );
        assert_eq!(default_unsupported.fit, LocalModelFit::NotRecommended);
        assert!(default_unsupported.message.contains("Apple Unified 32 GB"));

        let default_ready = assess_local_model_fit_for_hardware(
            DEFAULT_LOCAL_MODEL,
            HardwareProfile::AppleUnified,
            Some(48),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            false,
        );
        assert_eq!(default_ready.fit, LocalModelFit::Recommended);

        let glm = assess_local_model_fit_for_hardware(
            "glm-4.7-flash",
            HardwareProfile::AppleUnified,
            Some(48),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            false,
        );
        assert_eq!(glm.fit, LocalModelFit::Recommended);
        assert!(glm.requires_test);

        let unsupported_context = assess_local_model_fit_for_hardware(
            FAST_ROUTER_MODEL,
            HardwareProfile::AppleUnified,
            Some(16),
            200_000,
            false,
        );
        assert_eq!(unsupported_context.fit, LocalModelFit::NotRecommended);
        assert_eq!(unsupported_context.context_compatible, Some(false));
        assert!(
            unsupported_context
                .message
                .contains("will not lower context")
        );

        let custom = assess_local_model_fit_for_hardware(
            "my-model:latest",
            HardwareProfile::AppleUnified,
            Some(48),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            false,
        );
        assert_eq!(custom.fit, LocalModelFit::Unknown);
        assert!(custom.message.contains("keep the current model"));

        let mlx_on_nvidia = assess_local_model_fit_for_hardware(
            DEFAULT_LOCAL_MODEL,
            HardwareProfile::NvidiaVram,
            Some(96),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            false,
        );
        assert_eq!(mlx_on_nvidia.fit, LocalModelFit::NotRecommended);
    }

    #[test]
    fn catalog_covers_the_researched_memory_tiers() {
        let profiles = local_model_profiles();
        assert!(profiles.iter().any(|profile| {
            profile.id == FAST_ROUTER_MODEL
                && profile.quantization == "Q4_K_M"
                && profile.license.as_deref() == Some("LFM Open License 1.0")
        }));
        assert!(profiles.iter().any(|profile| {
            profile.id == "gemma4:12b-mlx" && profile.package_size_bytes == Some(7_700_000_000)
        }));
        assert!(profiles.iter().any(|profile| {
            profile.id == "gemma3:12b"
                && profile.minimum_memory_gb == 20
                && profile.recommended_memory_gb == 24
        }));
        assert!(profiles.iter().any(|profile| {
            profile.id == "llama3.3:70b"
                && profile.minimum_memory_gb == 80
                && profile.recommended_memory_gb == 96
        }));
        assert!(profiles.iter().any(|profile| {
            profile.id == "qwen3:32b"
                && profile.minimum_memory_gb == 40
                && profile.recommended_memory_gb == 48
        }));
        assert!(profiles.iter().any(|profile| {
            profile.id == "qwen3:235b"
                && profile.minimum_memory_gb == 176
                && profile.recommended_memory_gb == 192
        }));
        assert_eq!(
            local_model_profile(DEFAULT_LOCAL_MODEL).and_then(|profile| profile.package_size_bytes),
            Some(22_000_000_000)
        );
    }

    #[test]
    fn conservative_apple_shortlists_cover_each_budget_without_unsafe_models() {
        let setup = LocalSetupSettings::default();
        for budget in [16, 24, 32, 48, 64, 96, 192] {
            let shortlist = local_model_recommendations(
                HardwareProfile::AppleUnified,
                Some(budget),
                DEFAULT_CONTEXT_WINDOW_TOKENS,
                &setup,
                &[],
            );
            let defaults = shortlist
                .iter()
                .filter(|model| model.default_recommended)
                .collect::<Vec<_>>();
            assert!(
                defaults.len() <= 3,
                "{budget} GB exposed more than three Manager models"
            );
            assert!(defaults.iter().all(|model| {
                model.fit.fit == LocalModelFit::Recommended
                    && model.fit.context_window_tokens == DEFAULT_CONTEXT_WINDOW_TOKENS
                    && model.fit.context_compatible == Some(true)
            }));
            let ids = defaults
                .iter()
                .map(|model| model.profile.id.as_str())
                .collect::<Vec<_>>();
            let unique = ids
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(ids.len(), unique.len(), "{budget} GB duplicated a model");
        }

        let apple_16 = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(16),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        assert_eq!(
            apple_16
                .iter()
                .filter(|model| model.default_recommended)
                .map(|model| model.profile.id.as_str())
                .collect::<Vec<_>>(),
            vec![FAST_ROUTER_MODEL]
        );

        let apple_24 = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(24),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        let apple_24_ids = apple_24
            .iter()
            .filter(|model| model.default_recommended)
            .map(|model| model.profile.id.as_str())
            .collect::<Vec<_>>();
        assert!(apple_24_ids.contains(&FAST_ROUTER_MODEL));
        assert!(apple_24_ids.contains(&"gemma4:12b-mlx"));
        assert!(!apple_24_ids.contains(&"qwen3:14b"));
        assert!(!apple_24_ids.contains(&"gpt-oss:20b"));

        for budget in [32, 48, 64] {
            let shortlist = local_model_recommendations(
                HardwareProfile::AppleUnified,
                Some(budget),
                DEFAULT_CONTEXT_WINDOW_TOKENS,
                &setup,
                &[],
            );
            assert!(
                shortlist.iter().any(|model| {
                    model.default_recommended && model.profile.id == FAST_ROUTER_MODEL
                }),
                "{budget} GB should retain one fast routing role alongside stronger choices"
            );
        }

        let apple_48 = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(48),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        assert!(
            apple_48.iter().any(|model| {
                model.default_recommended && model.profile.id == DEFAULT_LOCAL_MODEL
            })
        );
    }

    #[test]
    fn nvidia_and_cpu_profiles_use_their_own_primary_limits() {
        let setup = LocalSetupSettings::default();
        let nvidia_24 = local_model_recommendations(
            HardwareProfile::NvidiaVram,
            Some(24),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        assert!(nvidia_24.iter().all(|model| {
            !model.default_recommended || model.profile.id != DEFAULT_LOCAL_MODEL
        }));
        assert!(
            nvidia_24.iter().any(|model| {
                model.default_recommended && model.profile.id == FAST_ROUTER_MODEL
            })
        );
        let cpu_16 = local_model_recommendations(
            HardwareProfile::CpuOnly,
            Some(16),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        assert!(cpu_16.iter().all(|model| !model.default_recommended));
        let cpu_24 = local_model_recommendations(
            HardwareProfile::CpuOnly,
            Some(24),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        assert!(
            cpu_24.iter().any(|model| {
                model.default_recommended && model.profile.id == FAST_ROUTER_MODEL
            })
        );
    }

    #[test]
    fn advanced_models_require_matching_real_32k_verification() {
        let mut setup = LocalSetupSettings::default();
        let llama_artifact = LocalModelArtifact {
            resolved_model: "llama3.3:70b".to_owned(),
            digest: "sha256:llama-q4-test".to_owned(),
            quantization: "Q4_K_M".to_owned(),
        };
        let before = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(96),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[],
        );
        assert!(before.iter().any(|model| {
            model.profile.id == "llama3.3:70b" && model.advanced_only && model.fit.requires_test
        }));
        setup.record_verification(LocalModelVerification {
            model: "llama3.3:70b".to_owned(),
            resolved_model: "llama3.3:70b".to_owned(),
            digest: llama_artifact.digest.clone(),
            quantization: "Q4_K_M".to_owned(),
            hardware_profile: HardwareProfile::AppleUnified,
            effective_capacity_gb: 96,
            context_window_tokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
            verified_at_unix_seconds: 1,
        });
        let after = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(96),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            std::slice::from_ref(&llama_artifact),
        );
        assert!(after.iter().any(|model| {
            model.profile.id == "llama3.3:70b" && model.advanced_only && model.fit.verified_at_32k
        }));

        let wrong_context = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(96),
            65_536,
            &setup,
            std::slice::from_ref(&llama_artifact),
        );
        assert!(
            wrong_context
                .iter()
                .any(|model| { model.profile.id == "llama3.3:70b" && model.advanced_only })
        );
        let wrong_hardware = local_model_recommendations(
            HardwareProfile::NvidiaVram,
            Some(96),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            std::slice::from_ref(&llama_artifact),
        );
        assert!(
            wrong_hardware
                .iter()
                .any(|model| { model.profile.id == "llama3.3:70b" && model.advanced_only })
        );

        let mut wrong_quantization_setup = LocalSetupSettings::default();
        wrong_quantization_setup.record_verification(LocalModelVerification {
            model: "llama3.3:70b".to_owned(),
            resolved_model: "llama3.3:70b".to_owned(),
            digest: llama_artifact.digest.clone(),
            quantization: "Q8_0".to_owned(),
            hardware_profile: HardwareProfile::AppleUnified,
            effective_capacity_gb: 96,
            context_window_tokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
            verified_at_unix_seconds: 2,
        });
        let wrong_quantization = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(96),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &wrong_quantization_setup,
            std::slice::from_ref(&llama_artifact),
        );
        assert!(wrong_quantization.iter().any(|model| {
            model.profile.id == "llama3.3:70b" && model.advanced_only && model.fit.requires_test
        }));

        let changed_artifact = LocalModelArtifact {
            digest: "sha256:llama-q4-new-pull".to_owned(),
            ..llama_artifact.clone()
        };
        let changed_pull = local_model_recommendations(
            HardwareProfile::AppleUnified,
            Some(96),
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            &setup,
            &[changed_artifact],
        );
        assert!(changed_pull.iter().any(|model| {
            model.profile.id == "llama3.3:70b" && model.advanced_only && model.fit.requires_test
        }));

        let mut legacy_alias_setup = LocalSetupSettings::default();
        legacy_alias_setup.record_verification(LocalModelVerification {
            model: "glm-4.7-flash".to_owned(),
            resolved_model: "glm-4.7-flash".to_owned(),
            digest: "sha256:legacy".to_owned(),
            quantization: "Q4_K_M".to_owned(),
            hardware_profile: HardwareProfile::AppleUnified,
            effective_capacity_gb: 48,
            context_window_tokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
            verified_at_unix_seconds: 3,
        });
        let latest_artifact = LocalModelArtifact {
            resolved_model: "glm-4.7-flash:latest".to_owned(),
            digest: "sha256:legacy".to_owned(),
            quantization: "Q4_K_M".to_owned(),
        };
        assert!(
            legacy_alias_setup
                .matching_verification(
                    "glm-4.7-flash:latest",
                    &HardwareProfile::AppleUnified,
                    Some(48),
                    DEFAULT_CONTEXT_WINDOW_TOKENS,
                    Some(&latest_artifact),
                )
                .is_none()
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
        let mut legacy = default_user_config().unwrap();
        legacy.version = 2;
        legacy.brains.push(test_brain("test"));
        let everyday = legacy.brains.first_mut().unwrap();
        everyday.write_policy = WritePolicy::ReadOnly;
        everyday.write_directories.clear();
        fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

        let migrated = load_or_migrate_config(&path).unwrap();
        let everyday = migrated
            .brains
            .iter()
            .find(|brain| brain.id == "test")
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
        let mut config = default_user_config().unwrap();
        config.version = CURRENT_CONFIG_VERSION + 1;
        let contents = serde_json::to_vec(&config).unwrap();
        fs::write(&path, &contents).unwrap();

        let error = load_or_migrate_config(&path).unwrap_err();
        assert!(error.to_string().contains("newer than this app supports"));
        assert_eq!(fs::read(&path).unwrap(), contents);

        fs::remove_dir_all(directory).unwrap();
    }
}
