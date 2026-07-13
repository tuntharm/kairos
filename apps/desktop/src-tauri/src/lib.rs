mod storage;
mod system;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use globset::{Glob, GlobSetBuilder};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, State, WindowEvent,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as AutoLaunchExt};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, GlobalShortcutExt, Shortcut, ShortcutState,
};

use kairos_core::{
    AccessDisposition, AppSettings, BrainRecord, BrainRole, BriefAnswer, CROSS_BRAIN_PULSE_QUERY,
    ChatAnswer, ConfirmedNoteWrite, ContentDestination, ContextPack, ConversationTurn,
    DEFAULT_CONTEXT_WINDOW_TOKENS, EgressPolicy, GraphBrainSource, GraphBuildOptions, GraphCluster,
    GraphDiagnostic, GraphIndex, GraphNode, GraphNodeKind, HardwareProfile, InferenceSettings,
    KairosConfig, LocalModelArtifact, LocalModelChoice, LocalModelFitAssessment, LocalModelProfile,
    LocalModelSettings, LocalModelVerification, LocalSetupSettings, MEMORY_BUDGET_PRESETS_GB,
    MemoryBudgetMode, NoteWriteConfirmation, NoteWriteKind, NoteWriteProposal, NoteWriteRequest,
    OllamaPullProgress, ProviderConfig, ProviderKind, SummonTarget, WritePolicy,
    WriteProposalOptions, WriteProposalStore, assess_local_model_fit_for_hardware, build_context,
    build_graph_index, canonicalize_allowed_file, chat_with_anthropic_api, chat_with_claude_cli,
    chat_with_codex_cli, chat_with_ollama, chat_with_openai_api, default_brain_read_policy,
    default_config_path, default_tharm_config, enforce_content_egress, evaluate_access,
    load_or_migrate_config, local_model_choices, local_model_profile, local_model_recommendations,
    ollama_status, pull_ollama_model as pull_model_from_ollama, render_chat_prompt,
    stream_chat_with_ollama, synthesize_ollama, test_ollama_model as test_local_ollama_model,
    write_config,
};

const KEYCHAIN_SERVICE: &str = "com.tharm.kairos";
const CLOUD_PREVIEW_TTL: Duration = Duration::from_secs(10 * 60);
const SCOPED_EXECUTION_GRANT_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_GRAPH_SCAN_FILES: usize = 2_000;
const MAX_BRAIN_INSPECTION_FILES: usize = 2_000;
const MAX_TEMP_ATTACHMENTS: usize = 5;
const MAX_TEMP_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TEMP_ATTACHMENT_CHARS: usize = 16_000;

#[derive(Default)]
struct AppState {
    folder_selections: Mutex<HashMap<String, PathBuf>>,
    cloud_previews: Mutex<HashMap<String, PendingCloudPreview>>,
    scoped_execution_grants: Mutex<HashMap<String, ScopedExecutionGrant>>,
    write_proposals: Mutex<WriteProposalStore>,
    temporary_attachments: Mutex<HashMap<String, TemporaryAttachment>>,
    cancelled_pulls: Mutex<HashSet<String>>,
}

#[derive(Clone)]
struct PendingCloudPreview {
    provider_id: String,
    message: String,
    brain_override: Option<String>,
    history: Vec<ConversationTurn>,
    context: ContextPack,
    composed_message: String,
    attachment_ids: Vec<String>,
    created_at: SystemTime,
}

/// An opaque, native-issued session grant. It is deliberately not a Mac, shell,
/// deletion, private-note, or egress grant; it only proves the user selected a
/// bounded registered-brain scope for this session.
#[derive(Clone)]
struct ScopedExecutionGrant {
    session_id: String,
    provider_id: String,
    brain_id: String,
    expires_at: SystemTime,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopedExecutionGrantView {
    token: String,
    brain_id: String,
    expires_at: String,
}

#[derive(Clone)]
struct TemporaryAttachment {
    id: String,
    name: String,
    size: u64,
    text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    config_path: String,
    initialized: bool,
    model: ModelStatus,
    app: AppSettings,
    inference: InferenceSettings,
    providers: Vec<ProviderStatus>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ModelStatus {
    endpoint: String,
    selected_model: String,
    resolved_model: Option<String>,
    context_window_tokens: u32,
    running: bool,
    selected_model_installed: bool,
    installed_models: Vec<String>,
    setup_message: Option<String>,
    choices: Vec<LocalModelChoice>,
    selected_model_fit: LocalModelFitAssessment,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalBrief {
    answer: BriefAnswer,
    context: ContextPack,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalSetupStatus {
    ollama_installed: bool,
    running: bool,
    endpoint: String,
    detected_memory_gb: Option<u16>,
    detected_hardware_profile: HardwareProfile,
    selected_hardware_profile: HardwareProfile,
    effective_hardware_profile: HardwareProfile,
    hardware_profile_label: String,
    primary_fit_limit_gb: Option<u16>,
    primary_fit_limit_label: String,
    hardware_planning_override: bool,
    available_disk_gb: Option<u64>,
    context_window_tokens: u32,
    memory_budget_mode: MemoryBudgetMode,
    memory_budget_gb: Option<u16>,
    effective_memory_budget_gb: Option<u16>,
    memory_budget_message: String,
    memory_budget_planning_only: bool,
    selected_model_installed: bool,
    setup_message: Option<String>,
    ollama_install_action: OllamaInstallAction,
    selected_model_fit: LocalModelFitAssessment,
    models: Vec<LocalSetupModel>,
    recommended_models: Vec<LocalSetupModel>,
    advanced_models: Vec<LocalSetupModel>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalSetupModel {
    id: String,
    label: String,
    role: String,
    installed: bool,
    download_size: String,
    memory_band: String,
    recommended_context: String,
    fit: LocalModelFitAssessment,
    variant: String,
    capabilities: Vec<String>,
    why_recommended: String,
    recommendation_rank: Option<u8>,
    default_recommended: bool,
    advanced_only: bool,
    verified_at_32k: bool,
}

/// The selected local-model planning budget. It is intentionally distinct
/// from detected hardware and never changes macOS/Ollama allocation by itself.
#[derive(Clone)]
struct MemoryBudgetStatus {
    mode: MemoryBudgetMode,
    configured_gb: Option<u16>,
    effective_gb: Option<u16>,
    planning_only: bool,
    message: String,
}

/// Resolved from a physical machine in Auto mode or from an explicitly
/// labelled manual planning profile. A manual profile never claims this Mac
/// owns the selected resource.
#[derive(Clone)]
struct HardwarePlan {
    detected_profile: HardwareProfile,
    selected_profile: HardwareProfile,
    effective_profile: HardwareProfile,
    primary_fit_limit_gb: Option<u16>,
    detected_primary_capacity_gb: Option<u16>,
    planning_override: bool,
    primary_fit_limit_label: String,
    profile_label: String,
}

/// A typed, fixed action for the setup UI—not a user-provided URL or shell
/// command. macOS still owns the user-approved app installation.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaInstallAction {
    command: String,
    label: String,
    official_url: String,
    needed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderStatus {
    id: String,
    kind: ProviderKind,
    label: String,
    enabled: bool,
    configured: bool,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrainView {
    id: String,
    name: String,
    role: BrainRole,
    root_path: String,
    enabled: bool,
    graph_enabled: bool,
    egress_policy: EgressPolicy,
    write_policy: WritePolicy,
    write_directories: Vec<String>,
    status: String,
    note_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FolderSelection {
    selection_token: String,
    display_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrainInspection {
    suggested_name: String,
    suggested_role: String,
    routing_hints: Vec<String>,
    note_count: usize,
    router_candidates: Vec<String>,
    context_candidates: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GraphSnapshot {
    nodes: Vec<GraphNode>,
    edges: Vec<kairos_core::GraphEdge>,
    clusters: Vec<GraphCluster>,
    diagnostics: Vec<GraphDiagnostic>,
    indexed_at: String,
    stale: bool,
    capped: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatPreview {
    preview_token: String,
    provider_id: String,
    provider_label: String,
    route: kairos_core::RouteResult,
    context: ContextPack,
    outgoing_summary: String,
    attachment_names: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TemporaryAttachmentInfo {
    id: String,
    name: String,
    size: u64,
    kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatResponse {
    answer: String,
    context: ContextPack,
    provider_id: String,
    provider_label: String,
    session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatProviderRequest {
    message: String,
    brain_override: Option<String>,
    provider_id: Option<String>,
    confirmed_external: bool,
    preview_token: Option<String>,
    history: Option<Vec<ConversationTurn>>,
    session_id: Option<String>,
    attachment_ids: Option<Vec<String>>,
    turn_id: Option<String>,
    execution_grant: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateBrainRequest {
    selection_token: String,
    name: String,
    role: String,
    routing_hints: Vec<String>,
    egress_policy: String,
    write_policy: String,
    graph_enabled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatDelta {
    turn_id: String,
    delta: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OllamaPullEvent {
    model: String,
    #[serde(flatten)]
    progress: OllamaPullProgress,
}

fn config_path() -> Result<PathBuf, String> {
    default_config_path().map_err(|error| error.to_string())
}

fn load_app_config(path: &Path) -> Result<KairosConfig, String> {
    load_or_migrate_config(path).map_err(|error| error.to_string())
}

fn load_or_create_config() -> Result<(PathBuf, KairosConfig), String> {
    let path = config_path()?;
    if path.exists() {
        return Ok((path.clone(), load_app_config(&path)?));
    }
    let config = default_tharm_config().map_err(|error| error.to_string())?;
    write_config(&path, &config, false).map_err(|error| error.to_string())?;
    Ok((path, config))
}

fn resolve_hardware_plan(
    settings: &LocalSetupSettings,
    capability: &system::MachineCapability,
) -> HardwarePlan {
    let selected_profile = settings.hardware_profile;
    let detected_profile = capability.detected_hardware_profile;
    let configured_budget = match settings.resolved_memory_budget_mode() {
        MemoryBudgetMode::Auto => None,
        MemoryBudgetMode::Preset | MemoryBudgetMode::Custom => settings.memory_budget_gb,
    };
    if selected_profile == HardwareProfile::Auto {
        let primary_fit_limit_gb = match (capability.primary_fit_capacity_gib, configured_budget) {
            (Some(detected), Some(cap)) => Some(detected.min(cap)),
            (detected, None) => detected,
            (None, _) => None,
        };
        return HardwarePlan {
            detected_profile,
            selected_profile,
            effective_profile: detected_profile,
            primary_fit_limit_gb,
            detected_primary_capacity_gb: capability.primary_fit_capacity_gib,
            planning_override: false,
            primary_fit_limit_label: detected_profile.primary_resource_label().to_owned(),
            profile_label: format!("{} · auto-detected", detected_profile.label()),
        };
    }
    HardwarePlan {
        detected_profile,
        selected_profile,
        effective_profile: selected_profile,
        // A manual profile is intentionally a planning target. It does not
        // consume or invent hardware on this Mac, and needs an explicit cap.
        primary_fit_limit_gb: configured_budget,
        detected_primary_capacity_gb: capability.primary_fit_capacity_gib,
        planning_override: true,
        primary_fit_limit_label: selected_profile.primary_resource_label().to_owned(),
        profile_label: format!("{} · manual planning override", selected_profile.label()),
    }
}

fn memory_budget_status(
    settings: &LocalSetupSettings,
    hardware: &HardwarePlan,
) -> MemoryBudgetStatus {
    let mode = settings.resolved_memory_budget_mode();
    let configured_gb = match &mode {
        MemoryBudgetMode::Auto => None,
        MemoryBudgetMode::Preset | MemoryBudgetMode::Custom => settings.memory_budget_gb,
    };
    let effective_gb = hardware.primary_fit_limit_gb;
    let message = if hardware.planning_override {
        match configured_gb {
            Some(selected) => format!(
                "Manual {} planning override at {selected} GB. This is not detected hardware and never changes your selected model or 32K target.",
                hardware.effective_profile.label()
            ),
            None => format!(
                "Manual {} planning override selected. Choose a Memory Budget to compare models; Kairos will not claim this Mac has that hardware.",
                hardware.effective_profile.label()
            ),
        }
    } else {
        match (configured_gb, hardware.detected_primary_capacity_gb, effective_gb) {
            (None, Some(detected), _) => format!(
                "Auto uses the detected {detected} GB {} limit. It is planning guidance only and never changes Ollama's allocation, model, or 32K target.",
                hardware.primary_fit_limit_label
            ),
            (Some(cap), Some(detected), Some(effective)) if cap > detected => format!(
                "Your {cap} GB preference cap is above the detected {detected} GB {} limit, so Kairos conservatively assesses {effective} GB. It does not add hardware or switch models.",
                hardware.primary_fit_limit_label
            ),
            (Some(cap), Some(detected), Some(effective)) => format!(
                "Your {cap} GB preference cap limits the detected {detected} GB {} to {effective} GB for recommendations only. It does not reserve memory or switch models.",
                hardware.primary_fit_limit_label
            ),
            _ => "Auto is waiting to detect the primary local-model resource. Kairos will not change the selected model or context.".to_owned(),
        }
    };
    MemoryBudgetStatus {
        mode,
        configured_gb,
        effective_gb,
        planning_only: true,
        message,
    }
}

async fn model_status(
    settings: &LocalModelSettings,
    local_setup: &LocalSetupSettings,
    capability: &system::MachineCapability,
) -> ModelStatus {
    let readiness = ollama_status(settings).await;
    let hardware = resolve_hardware_plan(local_setup, capability);
    let artifacts = local_model_artifacts(&readiness);
    let selected_artifact = readiness
        .resolved_model
        .as_deref()
        .and_then(|model| artifact_for_model(&artifacts, model));
    let verified = local_setup
        .matching_verification(
            &settings.selected_model,
            &hardware.effective_profile,
            hardware.primary_fit_limit_gb,
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            selected_artifact,
        )
        .is_some();
    ModelStatus {
        endpoint: readiness.endpoint,
        selected_model: readiness.selected_model,
        resolved_model: readiness.resolved_model,
        context_window_tokens: settings.context_window_tokens,
        running: readiness.running,
        selected_model_installed: readiness.selected_model_installed,
        installed_models: readiness.installed_models,
        setup_message: readiness.setup_message,
        choices: local_model_choices(&settings.selected_model),
        selected_model_fit: assess_local_model_fit_for_hardware(
            &settings.selected_model,
            hardware.effective_profile,
            hardware.primary_fit_limit_gb,
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            verified,
        ),
    }
}

async fn status() -> Result<AppStatus, String> {
    let path = config_path()?;
    let initialized = path.exists();
    let config = if initialized {
        Some(load_app_config(&path)?)
    } else {
        None
    };
    let settings = config
        .as_ref()
        .map(|config| config.local_model.clone())
        .unwrap_or_default();
    let local_setup = config
        .as_ref()
        .map(|config| config.local_setup.clone())
        .unwrap_or_default();
    let capability = system::machine_capability();
    Ok(AppStatus {
        config_path: path.display().to_string(),
        initialized,
        model: model_status(&settings, &local_setup, &capability).await,
        app: config
            .as_ref()
            .map(|config| config.app.clone())
            .unwrap_or_default(),
        inference: config
            .as_ref()
            .map(|config| config.inference.clone())
            .unwrap_or_default(),
        providers: config.as_ref().map(provider_statuses).unwrap_or_default(),
    })
}

fn friendly_model_size(size_bytes: Option<u64>, profile: &LocalModelProfile) -> String {
    if let Some(bytes) = size_bytes {
        let gib = bytes as f64 / 1024_f64.powi(3);
        return format!("{gib:.1} GB installed");
    }
    match profile.package_size_bytes {
        Some(bytes) => {
            let gb = bytes as f64 / 1_000_000_000_f64;
            if (gb - gb.round()).abs() < f64::EPSILON {
                format!("~{} GB via Ollama", gb.round())
            } else {
                format!("~{gb:.1} GB via Ollama")
            }
        }
        None => "Size reported by Ollama during download".to_owned(),
    }
}

fn friendly_context_window(tokens: u32) -> String {
    if tokens.is_multiple_of(1024) {
        format!("{}K", tokens / 1024)
    } else if tokens.is_multiple_of(1_000) {
        format!("{}K", tokens / 1_000)
    } else {
        format!("{tokens} tokens")
    }
}

fn same_ollama_model_id(left: &str, right: &str) -> bool {
    left.trim().trim_end_matches(":latest") == right.trim().trim_end_matches(":latest")
}

fn local_model_artifact_from_installed(
    installed: &kairos_core::InstalledModel,
) -> Option<LocalModelArtifact> {
    let digest = installed.digest.as_ref()?.trim();
    let quantization = installed.quantization.as_ref()?.trim();
    if installed.id.trim().is_empty() || digest.is_empty() || quantization.is_empty() {
        return None;
    }
    Some(LocalModelArtifact {
        resolved_model: installed.id.clone(),
        digest: digest.to_owned(),
        quantization: quantization.to_owned(),
    })
}

fn local_model_artifacts(readiness: &kairos_core::OllamaStatus) -> Vec<LocalModelArtifact> {
    readiness
        .installed_model_sizes
        .iter()
        .filter_map(local_model_artifact_from_installed)
        .collect()
}

fn artifact_for_model<'a>(
    artifacts: &'a [LocalModelArtifact],
    model: &str,
) -> Option<&'a LocalModelArtifact> {
    artifacts
        .iter()
        // Verification requires the exact installed tag, not merely the
        // `:latest` family alias used elsewhere for selection convenience.
        .find(|artifact| artifact.resolved_model == model.trim())
}

fn local_setup_model_from_recommendation(
    recommendation: kairos_core::LocalModelRecommendation,
    readiness: &kairos_core::OllamaStatus,
) -> LocalSetupModel {
    let profile = recommendation.profile;
    let installed_model = readiness
        .installed_model_sizes
        .iter()
        .find(|installed| same_ollama_model_id(&installed.id, &profile.id));
    let installed = installed_model.is_some();
    LocalSetupModel {
        id: profile.id.clone(),
        label: profile.label.clone(),
        role: profile.role.clone(),
        installed,
        download_size: friendly_model_size(
            installed_model.and_then(|installed| installed.size_bytes),
            &profile,
        ),
        // Deliberately not a RAM/VRAM claim. The fit assessment is the sole
        // source of hardware guidance and carries the conservative 32K tier.
        memory_band: "Fit assessed separately for the selected hardware profile".to_owned(),
        recommended_context: format!(
            "32K target · supports up to {}",
            profile
                .maximum_context_tokens
                .map(friendly_context_window)
                .unwrap_or_else(|| "its Ollama-reported limit".to_owned())
        ),
        fit: recommendation.fit.clone(),
        variant: profile.quantization.clone(),
        capabilities: profile.capabilities.clone(),
        why_recommended: recommendation.why_recommended.clone(),
        recommendation_rank: recommendation.recommendation_rank,
        default_recommended: recommendation.default_recommended,
        advanced_only: recommendation.advanced_only,
        verified_at_32k: recommendation.fit.verified_at_32k,
    }
}

async fn local_setup() -> Result<LocalSetupStatus, String> {
    let path = config_path()?;
    let (settings, setup_settings) = if path.exists() {
        let config = load_app_config(&path)?;
        (config.local_model, config.local_setup)
    } else {
        (LocalModelSettings::default(), LocalSetupSettings::default())
    };
    let readiness = ollama_status(&settings).await;
    let capability = system::machine_capability();
    let hardware = resolve_hardware_plan(&setup_settings, &capability);
    let memory_budget = memory_budget_status(&setup_settings, &hardware);
    let artifacts = local_model_artifacts(&readiness);
    let recommendations = local_model_recommendations(
        hardware.effective_profile,
        hardware.primary_fit_limit_gb,
        DEFAULT_CONTEXT_WINDOW_TOKENS,
        &setup_settings,
        &artifacts,
    );
    let models = recommendations
        .into_iter()
        .map(|recommendation| local_setup_model_from_recommendation(recommendation, &readiness))
        .collect::<Vec<_>>();
    let recommended_models = models
        .iter()
        .filter(|model| model.default_recommended && !model.advanced_only)
        .cloned()
        .collect::<Vec<_>>();
    let advanced_models = models
        .iter()
        .filter(|model| model.advanced_only)
        .cloned()
        .collect::<Vec<_>>();
    let ollama_installed = system::ollama_is_installed();
    let setup_message = if !ollama_installed {
        Some(
            "Ollama is not installed. Use Install Ollama, then return here and refresh.".to_owned(),
        )
    } else if !readiness.running {
        Some("Ollama is installed but not running. Open Ollama, then refresh Kairos.".to_owned())
    } else {
        readiness.setup_message
    };
    Ok(LocalSetupStatus {
        ollama_installed,
        running: readiness.running,
        endpoint: settings.ollama_endpoint,
        detected_memory_gb: capability.unified_memory_gib,
        detected_hardware_profile: hardware.detected_profile,
        selected_hardware_profile: hardware.selected_profile,
        effective_hardware_profile: hardware.effective_profile,
        hardware_profile_label: hardware.profile_label,
        primary_fit_limit_gb: hardware.primary_fit_limit_gb,
        primary_fit_limit_label: hardware.primary_fit_limit_label,
        hardware_planning_override: hardware.planning_override,
        available_disk_gb: capability
            .free_disk_bytes
            .map(|bytes| bytes / 1024_u64.pow(3)),
        context_window_tokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
        memory_budget_mode: memory_budget.mode,
        memory_budget_gb: memory_budget.configured_gb,
        effective_memory_budget_gb: memory_budget.effective_gb,
        memory_budget_message: memory_budget.message,
        memory_budget_planning_only: memory_budget.planning_only,
        selected_model_installed: readiness.selected_model_installed,
        setup_message,
        ollama_install_action: OllamaInstallAction {
            command: "open_ollama_install_page".to_owned(),
            label: "Install Ollama".to_owned(),
            official_url: system::OLLAMA_MACOS_DOWNLOAD_URL.to_owned(),
            needed: !ollama_installed,
        },
        selected_model_fit: assess_local_model_fit_for_hardware(
            &settings.selected_model,
            hardware.effective_profile,
            hardware.primary_fit_limit_gb,
            DEFAULT_CONTEXT_WINDOW_TOKENS,
            setup_settings
                .matching_verification(
                    &settings.selected_model,
                    &hardware.effective_profile,
                    hardware.primary_fit_limit_gb,
                    DEFAULT_CONTEXT_WINDOW_TOKENS,
                    readiness
                        .resolved_model
                        .as_deref()
                        .and_then(|model| artifact_for_model(&artifacts, model)),
                )
                .is_some(),
        ),
        models,
        recommended_models,
        advanced_models,
    })
}

#[tauri::command]
async fn app_status() -> Result<AppStatus, String> {
    status().await
}

#[tauri::command]
async fn local_setup_status() -> Result<LocalSetupStatus, String> {
    local_setup().await
}

/// Persist only the local-model planning budget, then return a fresh fit
/// assessment. This deliberately avoids saving an unrelated shortcut,
/// provider, or context-window draft when the user changes a single selector.
#[tauri::command]
async fn set_memory_budget(
    memory_budget_mode: MemoryBudgetMode,
    memory_budget_gb: Option<u16>,
) -> Result<LocalSetupStatus, String> {
    let (path, mut config) = load_or_create_config()?;
    config
        .local_setup
        .set_memory_budget(memory_budget_mode, memory_budget_gb)
        .map_err(|error| error.to_string())?;
    config.ensure_current_version();
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    local_setup().await
}

/// Persist only the selected planning profile. It never changes the local
/// model, its context setting, or the physical machine's allocation.
#[tauri::command]
async fn set_hardware_profile(
    hardware_profile: HardwareProfile,
) -> Result<LocalSetupStatus, String> {
    let (path, mut config) = load_or_create_config()?;
    config.local_setup.set_hardware_profile(hardware_profile);
    config.ensure_current_version();
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    local_setup().await
}

#[tauri::command]
async fn initialize_tharm_profile() -> Result<AppStatus, String> {
    let path = config_path()?;
    if !path.exists() {
        let config = default_tharm_config().map_err(|error| error.to_string())?;
        write_config(&path, &config, false).map_err(|error| error.to_string())?;
    }
    status().await
}

#[tauri::command]
async fn set_selected_model(model: String) -> Result<AppStatus, String> {
    let (path, mut config) = load_or_create_config()?;
    config
        .local_model
        .set_selected_model(&model)
        .map_err(|error| error.to_string())?;
    config.ensure_current_version();
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    status().await
}

#[tauri::command]
async fn pull_ollama_model(
    app: AppHandle,
    state: State<'_, AppState>,
    model: String,
) -> Result<LocalSetupStatus, String> {
    let (_path, config) = load_or_create_config()?;
    state
        .cancelled_pulls
        .lock()
        .map_err(|_| "Kairos download state is unavailable.".to_owned())?
        .remove(&model);
    let handle = app.clone();
    let cancellation_state = &state;
    let pull_model = model.clone();
    let event_model = model.clone();
    pull_model_from_ollama(&config.local_model, &model, move |progress| {
        let _ = handle.emit(
            "ollama-pull-progress",
            OllamaPullEvent {
                model: event_model.clone(),
                progress,
            },
        );
        !cancellation_state
            .cancelled_pulls
            .lock()
            .map(|cancelled| cancelled.contains(&pull_model))
            .unwrap_or(true)
    })
    .await
    .map_err(|error| error.to_string())?;
    // Downloading is intentionally not a model-selection action. The caller
    // must invoke `set_selected_model` explicitly after inspecting the fit and
    // test result; there is no silent fallback or automatic activation.
    local_setup().await
}

#[tauri::command]
fn cancel_ollama_pull(state: State<'_, AppState>, model: String) -> Result<(), String> {
    if model.trim().is_empty() || model.len() > 200 {
        return Err("Choose a valid model download to cancel.".to_owned());
    }
    state
        .cancelled_pulls
        .lock()
        .map_err(|_| "Kairos download state is unavailable.".to_owned())?
        .insert(model);
    Ok(())
}

#[tauri::command]
async fn test_ollama_model(
    model: String,
    context_window_tokens: Option<u32>,
) -> Result<kairos_core::OllamaModelTest, String> {
    if context_window_tokens.is_some_and(|tokens| tokens != DEFAULT_CONTEXT_WINDOW_TOKENS) {
        return Err("Kairos setup verification is fixed at 32K. It does not silently lower or substitute context.".to_owned());
    }
    let (path, mut config) = load_or_create_config()?;
    let mut result = test_local_ollama_model(&config.local_model, &model)
        .await
        .map_err(|error| error.to_string())?;
    let capability = system::machine_capability();
    let hardware = resolve_hardware_plan(&config.local_setup, &capability);
    let can_verify_current_plan = !hardware.planning_override
        && hardware.primary_fit_limit_gb.is_some()
        && hardware.primary_fit_limit_gb == capability.primary_fit_capacity_gib
        && hardware.effective_profile == capability.detected_hardware_profile;
    let artifact = match (
        result.installed_digest.as_deref().map(str::trim),
        result.installed_quantization.as_deref().map(str::trim),
    ) {
        (Some(digest), Some(quantization)) if !digest.is_empty() && !quantization.is_empty() => {
            Some(LocalModelArtifact {
                resolved_model: result.resolved_model.clone(),
                digest: digest.to_owned(),
                quantization: quantization.to_owned(),
            })
        }
        _ => None,
    };
    let catalog_quantization_matches = artifact.as_ref().is_some_and(|artifact| {
        local_model_profile(&result.resolved_model).is_none_or(|profile| {
            artifact
                .quantization
                .eq_ignore_ascii_case(profile.quantization.trim())
        })
    });
    if can_verify_current_plan && catalog_quantization_matches {
        let effective_capacity_gb = hardware.primary_fit_limit_gb.expect("checked above");
        let artifact = artifact.expect("checked by catalog_quantization_matches");
        config
            .local_setup
            .record_verification(LocalModelVerification {
                model: result.resolved_model.clone(),
                resolved_model: result.resolved_model.clone(),
                digest: artifact.digest,
                quantization: artifact.quantization,
                hardware_profile: hardware.effective_profile,
                effective_capacity_gb,
                context_window_tokens: DEFAULT_CONTEXT_WINDOW_TOKENS,
                verified_at_unix_seconds: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            });
        config.ensure_current_version();
        write_config(&path, &config, true).map_err(|error| error.to_string())?;
        result
            .message
            .push_str(" Kairos marked this exact local 32K configuration verified.");
    } else if artifact.is_none() {
        result.message.push_str(" Ollama did not provide an exact digest and quantization for this installed artifact, so Kairos did not mark it verified.");
    } else if !catalog_quantization_matches {
        result.message.push_str(" The installed artifact's quantization does not match Kairos's exact catalog profile, so it remains Advanced rather than being marked verified.");
    } else {
        result.message.push_str(" The load passed on this Mac, but Kairos did not mark the selected planning override or lower preference cap as verified.");
    }
    Ok(result)
}

#[tauri::command]
fn open_ollama_install_page() -> Result<(), String> {
    system::open_ollama_install_page()
}

#[tauri::command]
fn set_surface_mode(app: AppHandle, expanded: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Kairos main window is unavailable.".to_owned())?;
    let (width, height, min_width, min_height) = if expanded {
        (1_100.0, 820.0, 720.0, 600.0)
    } else {
        (680.0, 720.0, 480.0, 540.0)
    };
    window
        .set_min_size(Some(tauri::LogicalSize::new(min_width, min_height)))
        .map_err(|error| format!("could not resize Kairos: {error}"))?;
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| format!("could not resize Kairos: {error}"))
}

#[tauri::command]
fn brief_context() -> Result<ContextPack, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    build_context(&config, CROSS_BRAIN_PULSE_QUERY, None, &[], None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn brief_with_ollama() -> Result<LocalBrief, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let context = build_context(&config, CROSS_BRAIN_PULSE_QUERY, None, &[], None)
        .map_err(|error| error.to_string())?;
    enforce_content_egress(
        &config,
        &context.route,
        ContentDestination::LocalOllama,
        false,
    )
    .map_err(|error| error.to_string())?;
    let answer = synthesize_ollama(&config.local_model, &context)
        .await
        .map_err(|error| error.to_string())?;
    Ok(LocalBrief { answer, context })
}

fn provider_alias(provider_id: &str) -> &str {
    match provider_id {
        "ollama" | "ollama-local" => "ollama-local",
        "openai" | "openai-api" => "openai-api",
        "anthropic" | "anthropic-api" => "anthropic-api",
        "codex-cli" => "codex-cli",
        "claude-cli" => "claude-cli",
        other => other,
    }
}

fn provider_for_request(
    config: &KairosConfig,
    requested: Option<&str>,
) -> Result<ProviderConfig, String> {
    let id = requested
        .map(provider_alias)
        .unwrap_or(config.inference.active_provider_id.as_str());
    let provider = config
        .provider(id)
        .ok_or_else(|| format!("Kairos does not know the provider `{id}`."))?;
    if !provider.enabled {
        return Err(format!(
            "{} is not enabled yet. Configure it in Settings before using it.",
            provider.label
        ));
    }
    Ok(provider.clone())
}

fn provider_statuses(config: &KairosConfig) -> Vec<ProviderStatus> {
    config
        .inference
        .providers
        .iter()
        .map(|provider| {
            let (configured, detail) = match provider.kind {
                ProviderKind::Ollama => {
                    (true, "Loopback Ollama at http://localhost:11434".to_owned())
                }
                ProviderKind::OpenAi | ProviderKind::Anthropic => {
                    let secret = keychain_secret(&provider.id).is_ok();
                    let model = provider
                        .model
                        .as_deref()
                        .is_some_and(|model| !model.trim().is_empty());
                    (
                        secret && model,
                        if secret && model {
                            "API key is stored in macOS Keychain.".to_owned()
                        } else {
                            "Add an API key and model in Settings.".to_owned()
                        },
                    )
                }
                ProviderKind::CodexCli => (
                    system::cli_is_available("codex"),
                    "Uses an explicit, ephemeral read-only Codex CLI handoff.".to_owned(),
                ),
                ProviderKind::ClaudeCli => (
                    system::cli_is_available("claude"),
                    "Uses an explicit, ephemeral no-tools Claude Code handoff.".to_owned(),
                ),
            };
            ProviderStatus {
                id: provider.id.clone(),
                kind: provider.kind.clone(),
                label: provider.label.clone(),
                enabled: provider.enabled,
                configured,
                detail,
            }
        })
        .collect()
}

fn keychain_entry(provider_id: &str) -> Result<Entry, String> {
    Entry::new(KEYCHAIN_SERVICE, provider_alias(provider_id))
        .map_err(|error| format!("macOS Keychain is unavailable: {error}"))
}

fn keychain_secret(provider_id: &str) -> Result<String, String> {
    let secret = keychain_entry(provider_id)?.get_password().map_err(|_| {
        format!(
            "{} has no API key in macOS Keychain.",
            provider_alias(provider_id)
        )
    })?;
    if secret.trim().is_empty() {
        return Err("The saved API key is empty.".to_owned());
    }
    Ok(secret)
}

fn provider_model(provider: &ProviderConfig) -> Result<&str, String> {
    provider
        .model
        .as_deref()
        .filter(|model| !model.trim().is_empty())
        .ok_or_else(|| format!("Choose a model for {} in Settings.", provider.label))
}

fn require_external_preview_access(
    config: &KairosConfig,
    context: &ContextPack,
) -> Result<(), String> {
    enforce_content_egress(
        config,
        &context.route,
        ContentDestination::ExternalCloud,
        true,
    )
    .map_err(|error| error.to_string())
}

fn opaque_token(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{prefix}-{}-{nanos}", std::process::id())
}

fn truncate_attachment_text(text: &str) -> String {
    if text.len() <= MAX_TEMP_ATTACHMENT_CHARS {
        return text.to_owned();
    }
    let mut end = MAX_TEMP_ATTACHMENT_CHARS;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[temporary attachment truncated by Kairos]",
        &text[..end]
    )
}

fn extract_temporary_attachment(path: &Path) -> Result<TemporaryAttachment, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() > MAX_TEMP_ATTACHMENT_BYTES {
        return Err("Kairos accepts regular files up to 20 MB.".to_owned());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "The selected file needs a readable name.".to_owned())?
        .to_owned();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = match extension.as_str() {
        "md" | "txt" => {
            fs::read_to_string(path).map_err(|error| format!("could not read {name}: {error}"))?
        }
        "pdf" => {
            let command = system::known_program_path("pdftotext").ok_or_else(|| {
                "PDF extraction needs pdftotext on this Mac; the file was not attached.".to_owned()
            })?;
            let output = Command::new(command)
                .arg(path)
                .arg("-")
                .output()
                .map_err(|error| format!("could not extract {name}: {error}"))?;
            if !output.status.success() {
                return Err(format!("Kairos could not extract text from {name}."));
            }
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        "docx" => {
            let output = Command::new("/usr/bin/textutil")
                .args(["-convert", "txt", "-stdout"])
                .arg(path)
                .output()
                .map_err(|error| format!("could not extract {name}: {error}"))?;
            if !output.status.success() {
                return Err(format!("Kairos could not extract text from {name}."));
            }
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        _ => {
            return Err("Kairos accepts .md, .txt, .pdf, and .docx attachments.".to_owned());
        }
    };
    let text = truncate_attachment_text(&text.replace('\0', ""));
    if text.trim().is_empty() {
        return Err(format!("Kairos found no readable text in {name}."));
    }
    Ok(TemporaryAttachment {
        id: opaque_token("attachment"),
        name,
        size: metadata.len(),
        text,
    })
}

fn selected_attachments(
    state: &AppState,
    attachment_ids: Option<&[String]>,
) -> Result<Vec<TemporaryAttachment>, String> {
    let attachment_ids = attachment_ids.unwrap_or_default();
    if attachment_ids.len() > MAX_TEMP_ATTACHMENTS {
        return Err("Attach at most five temporary files at a time.".to_owned());
    }
    let attachments = state
        .temporary_attachments
        .lock()
        .map_err(|_| "Kairos temporary attachment state is unavailable.".to_owned())?;
    let mut selected = Vec::new();
    for id in attachment_ids {
        let attachment = attachments.get(id).cloned().ok_or_else(|| {
            "A temporary attachment is no longer available. Attach it again before sending."
                .to_owned()
        })?;
        selected.push(attachment);
    }
    Ok(selected)
}

fn compose_message_with_attachments(message: &str, attachments: &[TemporaryAttachment]) -> String {
    if attachments.is_empty() {
        return message.to_owned();
    }
    let attachment_text = attachments
        .iter()
        .map(|attachment| {
            format!(
                "<temporary_attachment name=\"{}\">\n{}\n</temporary_attachment>",
                attachment.name, attachment.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{message}\n\nTemporary attachments supplied by the user:\n{attachment_text}")
}

fn consume_temporary_attachments(state: &AppState, attachment_ids: &[String]) {
    if let Ok(mut attachments) = state.temporary_attachments.lock() {
        for id in attachment_ids {
            attachments.remove(id);
        }
    }
}

#[tauri::command]
async fn pick_temp_attachments(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<TemporaryAttachmentInfo>, String> {
    let handle = app.clone();
    let selected =
        tauri::async_runtime::spawn_blocking(move || handle.dialog().file().blocking_pick_files())
            .await
            .map_err(|error| format!("native attachment picker failed: {error}"))?;
    let Some(selected) = selected else {
        return Ok(Vec::new());
    };
    let remaining = MAX_TEMP_ATTACHMENTS.saturating_sub(
        state
            .temporary_attachments
            .lock()
            .map_err(|_| "Kairos temporary attachment state is unavailable.".to_owned())?
            .len(),
    );
    if remaining == 0 {
        return Err("Remove a temporary attachment before adding another one.".to_owned());
    }
    let mut extracted = Vec::new();
    for selected in selected.into_iter().take(remaining) {
        let path = selected
            .into_path()
            .map_err(|error| format!("selected attachment path is unavailable: {error}"))?;
        extracted.push(extract_temporary_attachment(&path)?);
    }
    let info = extracted
        .iter()
        .map(|attachment| TemporaryAttachmentInfo {
            id: attachment.id.clone(),
            name: attachment.name.clone(),
            size: attachment.size,
            kind: "temporary".to_owned(),
        })
        .collect::<Vec<_>>();
    let mut attachments = state
        .temporary_attachments
        .lock()
        .map_err(|_| "Kairos temporary attachment state is unavailable.".to_owned())?;
    for attachment in extracted {
        attachments.insert(attachment.id.clone(), attachment);
    }
    Ok(info)
}

#[tauri::command]
fn discard_temp_attachment(
    state: State<'_, AppState>,
    attachment_id: String,
) -> Result<(), String> {
    state
        .temporary_attachments
        .lock()
        .map_err(|_| "Kairos temporary attachment state is unavailable.".to_owned())?
        .remove(&attachment_id);
    Ok(())
}

#[tauri::command]
async fn chat_preview(
    state: State<'_, AppState>,
    message: String,
    brain_override: Option<String>,
    provider_id: Option<String>,
    history: Option<Vec<ConversationTurn>>,
    attachment_ids: Option<Vec<String>>,
) -> Result<ChatPreview, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let provider = provider_for_request(&config, provider_id.as_deref())?;
    if !provider.is_external() {
        return Err("Local Ollama does not require an external-provider preview.".to_owned());
    }
    let history = history.unwrap_or_default();
    let attachment_ids = attachment_ids.unwrap_or_default();
    let attachments = selected_attachments(&state, Some(&attachment_ids))?;
    let context = build_context(&config, &message, brain_override.as_deref(), &[], None)
        .map_err(|error| error.to_string())?;
    require_external_preview_access(&config, &context)?;
    let composed_message = compose_message_with_attachments(&message, &attachments);
    let outbound = render_chat_prompt(&context, &composed_message, &history);
    let token = opaque_token("cloud-preview");
    let pending = PendingCloudPreview {
        provider_id: provider.id.clone(),
        message: message.clone(),
        brain_override: brain_override.clone(),
        history,
        context: context.clone(),
        composed_message,
        attachment_ids: attachment_ids.clone(),
        created_at: SystemTime::now(),
    };
    state
        .cloud_previews
        .lock()
        .map_err(|_| "Kairos cloud-preview state is unavailable.".to_owned())?
        .insert(token.clone(), pending);
    Ok(ChatPreview {
        preview_token: token,
        provider_id: provider.id,
        provider_label: provider.label,
        route: context.route.clone(),
        context,
        outgoing_summary: outbound,
        attachment_names: attachments
            .iter()
            .map(|attachment| attachment.name.clone())
            .collect(),
    })
}

fn take_confirmed_preview(
    state: &AppState,
    token: Option<&str>,
    provider_id: &str,
    message: &str,
    brain_override: &Option<String>,
) -> Result<PendingCloudPreview, String> {
    let token = token.ok_or_else(|| {
        "Review this cloud turn first. Kairos requires its one-time confirmation token.".to_owned()
    })?;
    let mut previews = state
        .cloud_previews
        .lock()
        .map_err(|_| "Kairos cloud-preview state is unavailable.".to_owned())?;
    previews.retain(|_, preview| {
        preview
            .created_at
            .elapsed()
            .map(|elapsed| elapsed <= CLOUD_PREVIEW_TTL)
            .unwrap_or(false)
    });
    let preview = previews.remove(token).ok_or_else(|| {
        "This external preview has expired or was already used. Review the current turn again."
            .to_owned()
    })?;
    if preview.provider_id != provider_id
        || preview.message != message
        || preview.brain_override != *brain_override
    {
        return Err("The confirmed preview no longer matches this outbound turn.".to_owned());
    }
    Ok(preview)
}

#[tauri::command]
fn grant_scoped_execution(
    state: State<'_, AppState>,
    session_id: String,
    provider_id: String,
    brain_id: String,
) -> Result<ScopedExecutionGrantView, String> {
    if session_id.trim().is_empty() || session_id.len() > 256 {
        return Err("Start a local Kairos chat before granting scoped execution.".to_owned());
    }
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let provider = provider_for_request(&config, Some(&provider_id))?;
    let brain = config
        .brains
        .iter()
        .find(|brain| brain.id == brain_id && brain.enabled)
        .ok_or_else(|| "Choose an enabled registered brain for this session scope.".to_owned())?;
    if brain.write_policy == WritePolicy::Prohibited {
        return Err(
            "That brain prohibits note writes, so it cannot receive an execution grant.".to_owned(),
        );
    }
    let token = opaque_token("scoped-execution");
    let expires_at = SystemTime::now() + SCOPED_EXECUTION_GRANT_TTL;
    state
        .scoped_execution_grants
        .lock()
        .map_err(|_| "Kairos scoped-execution state is unavailable.".to_owned())?
        .insert(
            token.clone(),
            ScopedExecutionGrant {
                session_id,
                provider_id: provider.id,
                brain_id: brain.id.clone(),
                expires_at,
            },
        );
    Ok(ScopedExecutionGrantView {
        token,
        brain_id: brain.id.clone(),
        expires_at: chrono::DateTime::<chrono::Utc>::from(expires_at).to_rfc3339(),
    })
}

fn validate_scoped_execution_grant(
    state: &AppState,
    token: Option<&str>,
    session_id: Option<&str>,
    provider_id: &str,
    brain_id: Option<&str>,
) -> Result<(), String> {
    let token = token.ok_or_else(|| {
        "Full access requires a current native scoped-execution grant.".to_owned()
    })?;
    let mut grants = state
        .scoped_execution_grants
        .lock()
        .map_err(|_| "Kairos scoped-execution state is unavailable.".to_owned())?;
    grants.retain(|_, grant| grant.expires_at > SystemTime::now());
    let grant = grants.get(token).ok_or_else(|| {
        "This scoped-execution grant has expired. Grant it again for this session.".to_owned()
    })?;
    if Some(grant.session_id.as_str()) != session_id
        || grant.provider_id != provider_id
        || brain_id.is_some_and(|brain| brain != grant.brain_id)
    {
        return Err(
            "This scoped-execution grant does not match the current provider, chat, or brain."
                .to_owned(),
        );
    }
    Ok(())
}

async fn invoke_provider(
    provider: &ProviderConfig,
    config: &KairosConfig,
    context: &ContextPack,
    message: &str,
    history: &[ConversationTurn],
) -> Result<ChatAnswer, String> {
    match provider.kind {
        ProviderKind::Ollama => chat_with_ollama(&config.local_model, context, message, history)
            .await
            .map_err(|error| error.to_string()),
        ProviderKind::OpenAi => {
            let key = keychain_secret(&provider.id)?;
            chat_with_openai_api(&key, provider_model(provider)?, context, message, history)
                .await
                .map_err(|error| error.to_string())
        }
        ProviderKind::Anthropic => {
            let key = keychain_secret(&provider.id)?;
            chat_with_anthropic_api(&key, provider_model(provider)?, context, message, history)
                .await
                .map_err(|error| error.to_string())
        }
        ProviderKind::CodexCli => chat_with_codex_cli(context, message, history)
            .await
            .map_err(|error| error.to_string()),
        ProviderKind::ClaudeCli => chat_with_claude_cli(context, message, history)
            .await
            .map_err(|error| error.to_string()),
    }
}

fn store_chat_exchange(
    session_id: Option<String>,
    message: &str,
    answer: &ChatAnswer,
    provider: &ProviderConfig,
    context: &ContextPack,
) -> Result<String, String> {
    let path = storage::chat_archive_path()?;
    let mut archive = storage::load_chat_archive(&path)?;
    let session = if let Some(session_id) = session_id {
        archive
            .sessions
            .iter_mut()
            .find(|session| session.id == session_id)
            .ok_or_else(|| "The requested local chat session no longer exists.".to_owned())?
    } else {
        let created = storage::create_session(&mut archive, message);
        archive
            .sessions
            .iter_mut()
            .find(|session| session.id == created.id)
            .expect("new session exists")
    };
    storage::title_session_from_first_message(session, message);
    let routes = context
        .route
        .brains
        .iter()
        .map(|brain| brain.id.clone())
        .collect::<Vec<_>>();
    storage::append_message(
        session,
        "user",
        message.to_owned(),
        Vec::new(),
        Vec::new(),
        None,
        None,
        routes.clone(),
    );
    storage::append_message(
        session,
        "assistant",
        answer.content.clone(),
        answer.source_ids.clone(),
        Vec::new(),
        Some(provider.id.clone()),
        Some(provider.label.clone()),
        routes,
    );
    let id = session.id.clone();
    storage::persist_chat_archive(&path, &archive)?;
    Ok(id)
}

#[tauri::command]
async fn chat_with_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ChatProviderRequest,
) -> Result<ChatResponse, String> {
    let ChatProviderRequest {
        message,
        brain_override,
        provider_id,
        confirmed_external,
        preview_token,
        history,
        session_id,
        attachment_ids,
        turn_id,
        execution_grant,
    } = request;
    if message.trim().is_empty() || message.len() > 32_000 {
        return Err("A chat message must contain at most 32,000 characters.".to_owned());
    }
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let provider = provider_for_request(&config, provider_id.as_deref())?;
    if execution_grant.is_some() {
        validate_scoped_execution_grant(
            &state,
            execution_grant.as_deref(),
            session_id.as_deref(),
            &provider.id,
            brain_override.as_deref(),
        )?;
    }
    let attachment_ids = attachment_ids.unwrap_or_default();
    let (context, history, provider_message, consumed_attachment_ids) = if provider.is_external() {
        if !confirmed_external {
            return Err("Review this external turn before sending it.".to_owned());
        }
        let preview = take_confirmed_preview(
            &state,
            preview_token.as_deref(),
            &provider.id,
            &message,
            &brain_override,
        )?;
        require_external_preview_access(&config, &preview.context)?;
        (
            preview.context,
            preview.history,
            preview.composed_message,
            preview.attachment_ids,
        )
    } else {
        let history = history.unwrap_or_default();
        let attachments = selected_attachments(&state, Some(&attachment_ids))?;
        let context = build_context(&config, &message, brain_override.as_deref(), &[], None)
            .map_err(|error| error.to_string())?;
        enforce_content_egress(
            &config,
            &context.route,
            ContentDestination::LocalOllama,
            false,
        )
        .map_err(|error| error.to_string())?;
        (
            context,
            history,
            compose_message_with_attachments(&message, &attachments),
            attachment_ids,
        )
    };
    let answer = if provider.kind == ProviderKind::Ollama {
        let turn_id = turn_id.unwrap_or_else(|| opaque_token("chat-turn"));
        let handle = app.clone();
        stream_chat_with_ollama(
            &config.local_model,
            &context,
            &provider_message,
            &history,
            move |delta| {
                let _ = handle.emit(
                    "kairos-chat-delta",
                    ChatDelta {
                        turn_id: turn_id.clone(),
                        delta,
                    },
                );
            },
        )
        .await
        .map_err(|error| error.to_string())?
    } else {
        invoke_provider(&provider, &config, &context, &provider_message, &history).await?
    };
    let session_id = store_chat_exchange(session_id, &message, &answer, &provider, &context)?;
    consume_temporary_attachments(&state, &consumed_attachment_ids);
    Ok(ChatResponse {
        answer: answer.content,
        context,
        provider_id: provider.id,
        provider_label: provider.label,
        session_id,
    })
}

#[tauri::command]
fn list_chat_sessions(query: Option<String>) -> Result<Vec<storage::ChatSessionSummary>, String> {
    let archive = storage::load_chat_archive(&storage::chat_archive_path()?)?;
    Ok(storage::session_summaries(&archive, query.as_deref()))
}

#[tauri::command]
fn load_chat_session(session_id: String) -> Result<storage::StoredChatSession, String> {
    let archive = storage::load_chat_archive(&storage::chat_archive_path()?)?;
    archive
        .sessions
        .into_iter()
        .find(|session| session.id == session_id)
        .ok_or_else(|| "That chat no longer exists.".to_owned())
}

#[tauri::command]
fn create_chat_session() -> Result<storage::StoredChatSession, String> {
    let path = storage::chat_archive_path()?;
    let mut archive = storage::load_chat_archive(&path)?;
    let session = storage::create_session(&mut archive, "New conversation");
    storage::persist_chat_archive(&path, &archive)?;
    Ok(session)
}

#[tauri::command]
fn rename_chat_session(session_id: String, title: String) -> Result<(), String> {
    let path = storage::chat_archive_path()?;
    let mut archive = storage::load_chat_archive(&path)?;
    storage::rename_session(&mut archive, &session_id, &title)?;
    storage::persist_chat_archive(&path, &archive)
}

#[tauri::command]
fn delete_chat_session(session_id: String) -> Result<(), String> {
    let path = storage::chat_archive_path()?;
    let mut archive = storage::load_chat_archive(&path)?;
    storage::delete_session(&mut archive, &session_id)?;
    storage::persist_chat_archive(&path, &archive)
}

#[tauri::command]
fn clear_chat_history() -> Result<(), String> {
    let path = storage::chat_archive_path()?;
    storage::persist_chat_archive(&path, &storage::ChatArchive::default())
}

#[tauri::command]
async fn save_provider_settings(
    provider_id: String,
    model: Option<String>,
    api_key: Option<String>,
    enabled: bool,
    make_active: bool,
) -> Result<AppStatus, String> {
    let (path, mut config) = load_or_create_config()?;
    let requested = provider_alias(&provider_id);
    let provider = config
        .inference
        .providers
        .iter_mut()
        .find(|provider| provider.id == requested)
        .ok_or_else(|| format!("Kairos does not know the provider `{requested}`."))?;
    if matches!(
        provider.kind,
        ProviderKind::OpenAi | ProviderKind::Anthropic
    ) {
        if let Some(api_key) = api_key.as_deref() {
            if api_key.trim().is_empty() {
                return Err("API keys cannot be empty.".to_owned());
            }
            keychain_entry(&provider.id)?
                .set_password(api_key.trim())
                .map_err(|error| format!("could not save the key in macOS Keychain: {error}"))?;
        }
        if let Some(model) = model.as_deref() {
            if model.trim().is_empty() {
                return Err("Choose a non-empty cloud model identifier.".to_owned());
            }
            provider.model = Some(model.trim().to_owned());
        }
        if enabled && (keychain_secret(&provider.id).is_err() || provider_model(provider).is_err())
        {
            return Err(
                "Cloud providers need both a saved API key and a selected model.".to_owned(),
            );
        }
    }
    provider.enabled = enabled;
    if make_active {
        config.inference.active_provider_id = provider.id.clone();
    }
    config.ensure_current_version();
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    status().await
}

fn normalise_shortcut(shortcut: &str) -> Result<(String, Shortcut), String> {
    let normalized = shortcut.trim().replace("Option", "Alt").replace('⌥', "Alt");
    if normalized.is_empty() || normalized.len() > 80 {
        return Err("Choose a valid global shortcut.".to_owned());
    }
    let parsed = Shortcut::from_str(&normalized)
        .map_err(|error| format!("Kairos could not parse that shortcut: {error}"))?;
    Ok((normalized, parsed))
}

fn rebind_shortcut(app: &AppHandle, desired: Shortcut, previous: &str) -> Result<(), String> {
    let previous = normalise_shortcut(previous)
        .map(|(_, shortcut)| shortcut)
        .unwrap_or_else(|_| Shortcut::from_str("Alt+Space").expect("static shortcut"));
    app.global_shortcut()
        .unregister_all()
        .map_err(|error| format!("could not release the previous shortcut: {error}"))?;
    if let Err(error) = app.global_shortcut().register(desired) {
        let _ = app.global_shortcut().register(previous);
        return Err(format!(
            "Kairos could not register that shortcut. Another app may already use it: {error}"
        ));
    }
    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri maps the established flat WebView payload by name.
async fn save_app_preferences(
    app: AppHandle,
    shortcut: String,
    summon_target: String,
    launch_at_login: bool,
    close_to_hide: bool,
    keep_above_other_windows: Option<bool>,
    context_window_tokens: u32,
    memory_budget_gb: Option<u16>,
    memory_budget_mode: Option<MemoryBudgetMode>,
) -> Result<AppStatus, String> {
    let (path, mut config) = load_or_create_config()?;
    let (shortcut, parsed_shortcut) = normalise_shortcut(&shortcut)?;
    let summon_target = match summon_target.as_str() {
        "compact" | "compact_chat" => SummonTarget::CompactChat,
        "last-page" | "last_surface" => SummonTarget::LastSurface,
        _ => return Err("Choose Compact chat or Last open page.".to_owned()),
    };
    // Older WebViews send only `memoryBudgetGb`; preserve their meaning while
    // new callers round-trip the explicit Auto / Preset / Custom mode.
    let memory_budget_mode = memory_budget_mode.unwrap_or_else(|| match memory_budget_gb {
        None => MemoryBudgetMode::Auto,
        Some(budget) if MEMORY_BUDGET_PRESETS_GB.contains(&budget) => MemoryBudgetMode::Preset,
        Some(_) => MemoryBudgetMode::Custom,
    });
    config
        .local_setup
        .set_memory_budget(memory_budget_mode, memory_budget_gb)
        .map_err(|error| error.to_string())?;
    let previous_shortcut = config.app.summon_shortcut.clone();
    let previous_settings = config.local_model.clone();
    config.local_model.context_window_tokens = context_window_tokens;
    config
        .local_model
        .validate()
        .map_err(|error| error.to_string())?;
    rebind_shortcut(&app, parsed_shortcut, &previous_shortcut)?;
    if launch_at_login {
        app.autolaunch()
            .enable()
            .map_err(|error| format!("could not enable launch at login: {error}"))?;
    } else {
        app.autolaunch()
            .disable()
            .map_err(|error| format!("could not update launch at login: {error}"))?;
    }
    config.app = AppSettings {
        summon_shortcut: shortcut,
        summon_target,
        launch_at_login,
        close_to_hide,
        keep_above_other_windows: keep_above_other_windows.unwrap_or(false),
    };
    if let Some(window) = app.get_webview_window("main") {
        window
            .set_always_on_top(config.app.keep_above_other_windows)
            .map_err(|error| format!("could not update Kairos window layer: {error}"))?;
    }
    config.ensure_current_version();
    if let Err(error) = write_config(&path, &config, true) {
        config.local_model = previous_settings;
        let _ = rebind_shortcut(
            &app,
            normalise_shortcut(&previous_shortcut)
                .map(|(_, shortcut)| shortcut)
                .unwrap_or_else(|_| Shortcut::from_str("Alt+Space").expect("static shortcut")),
            &previous_shortcut,
        );
        return Err(error.to_string());
    }
    status().await
}

#[tauri::command]
async fn pick_brain_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<FolderSelection>, String> {
    let handle = app.clone();
    let selection =
        tauri::async_runtime::spawn_blocking(move || handle.dialog().file().blocking_pick_folder())
            .await
            .map_err(|error| format!("native folder picker failed: {error}"))?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let selected_path = selection
        .into_path()
        .map_err(|error| format!("selected folder path is unavailable: {error}"))?;
    let root = fs::canonicalize(&selected_path)
        .map_err(|error| format!("could not access the selected folder: {error}"))?;
    if !root.is_dir() {
        return Err("Choose a folder rather than a file.".to_owned());
    }
    let token = opaque_token("brain-folder");
    state
        .folder_selections
        .lock()
        .map_err(|_| "Kairos folder-selection state is unavailable.".to_owned())?
        .insert(token.clone(), root.clone());
    Ok(Some(FolderSelection {
        selection_token: token,
        display_path: root.display().to_string(),
    }))
}

fn selected_folder(state: &AppState, selection_token: &str) -> Result<PathBuf, String> {
    state
        .folder_selections
        .lock()
        .map_err(|_| "Kairos folder-selection state is unavailable.".to_owned())?
        .get(selection_token)
        .cloned()
        .ok_or_else(|| "This folder selection expired. Choose the folder again.".to_owned())
}

fn known_brain_paths(root: &Path) -> (Vec<String>, Vec<String>) {
    let routers = [
        "00_System/AI Brain Manager.md",
        "00_System/AI Second Brain Manager.md",
        "brain/00_System/Datter Brain Manager.md",
        "AGENTS.md",
        "README.md",
    ]
    .into_iter()
    .filter(|relative| root.join(relative).is_file())
    .map(str::to_owned)
    .collect();
    let context = [
        "00_System/Current Context.md",
        "00_System/PhD Current Context.md",
        "brain/00_System/Project Map.md",
        "00_System/Vault Map.md",
    ]
    .into_iter()
    .filter(|relative| root.join(relative).is_file())
    .map(str::to_owned)
    .collect();
    (routers, context)
}

fn bounded_markdown_count(root: &Path, maximum: usize) -> usize {
    let mut count = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        if count >= maximum {
            break;
        }
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if count >= maximum {
                break;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if file_type.is_dir() {
                if !name.starts_with('.') && name != "90_Private" && name != "Attachments" {
                    stack.push(path);
                }
            } else if file_type.is_file()
                && path.extension().and_then(|extension| extension.to_str()) == Some("md")
            {
                count += 1;
            }
        }
    }
    count
}

fn words_from_folder_name(root: &Path) -> Vec<String> {
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let mut words = name
        .split(|character: char| !character.is_alphanumeric())
        .map(|word| word.trim().to_lowercase())
        .filter(|word| word.len() >= 3)
        .collect::<Vec<_>>();
    words.sort();
    words.dedup();
    words.into_iter().take(6).collect()
}

#[tauri::command]
fn inspect_brain_folder(
    state: State<'_, AppState>,
    selection_token: String,
) -> Result<BrainInspection, String> {
    let root = selected_folder(&state, &selection_token)?;
    let (router_candidates, context_candidates) = known_brain_paths(&root);
    let suggested_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.replace(['-', '_'], " "))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "New Obsidian brain".to_owned());
    let suggested_role = if root.join(".obsidian").is_dir() {
        "Obsidian knowledge vault".to_owned()
    } else {
        "Registered knowledge folder".to_owned()
    };
    Ok(BrainInspection {
        suggested_name,
        suggested_role,
        routing_hints: words_from_folder_name(&root),
        note_count: bounded_markdown_count(&root, MAX_BRAIN_INSPECTION_FILES),
        router_candidates,
        context_candidates,
    })
}

fn slugify_brain_id(name: &str, existing_ids: &HashSet<String>) -> String {
    let mut id = name
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    let base = id.trim_matches('-');
    let base = if base.is_empty() { "brain" } else { base };
    let mut candidate = base.to_owned();
    let mut suffix = 2;
    while existing_ids.contains(&candidate) {
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
    candidate
}

fn parse_brain_role(role: &str) -> BrainRole {
    let role = role.to_lowercase();
    if role.contains("research") || role.contains("phd") {
        BrainRole::Phd
    } else if role.contains("project") {
        BrainRole::Project
    } else if role.contains("life") || role.contains("personal") {
        BrainRole::Everyday
    } else {
        BrainRole::Other
    }
}

fn parse_egress_policy(policy: &str) -> Result<EgressPolicy, String> {
    match policy {
        "local_only" => Ok(EgressPolicy::LocalOnly),
        "cloud_allowed" => Ok(EgressPolicy::CloudAllowed),
        "redact_required" => Ok(EgressPolicy::RedactRequired),
        _ => Err("Choose local_only, cloud_allowed, or redact_required.".to_owned()),
    }
}

fn parse_write_policy(policy: &str) -> Result<WritePolicy, String> {
    match policy {
        "readonly" | "read_only" => Ok(WritePolicy::ReadOnly),
        "confirm_every_write" | "propose_confirm" => Ok(WritePolicy::ProposeConfirm),
        "prohibited" => Ok(WritePolicy::Prohibited),
        _ => Err("Choose readonly, confirm_every_write, or prohibited.".to_owned()),
    }
}

fn write_directories_for_root(root: &Path) -> Vec<String> {
    [
        "01_Daily",
        "02_Projects",
        "03_People",
        "04_Areas",
        "05_Knowledge",
        "07_Decisions",
    ]
    .into_iter()
    .filter(|relative| root.join(relative).is_dir())
    .map(str::to_owned)
    .collect()
}

fn brain_view(brain: &BrainRecord) -> BrainView {
    let status = if !brain.root_path.is_dir() {
        "offline"
    } else if brain.write_policy == WritePolicy::Prohibited {
        "restricted"
    } else {
        "ready"
    };
    BrainView {
        id: brain.id.clone(),
        name: brain.name.clone(),
        role: brain.role.clone(),
        root_path: brain.root_path.display().to_string(),
        enabled: brain.enabled,
        graph_enabled: brain.graph_enabled,
        egress_policy: brain.egress_policy.clone(),
        write_policy: brain.write_policy.clone(),
        write_directories: brain.write_directories.clone(),
        status: status.to_owned(),
        note_count: bounded_markdown_count(&brain.root_path, MAX_BRAIN_INSPECTION_FILES),
    }
}

#[tauri::command]
fn list_brains() -> Result<Vec<BrainView>, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    Ok(config.brains.iter().map(brain_view).collect())
}

#[tauri::command]
fn create_brain(
    state: State<'_, AppState>,
    request: CreateBrainRequest,
) -> Result<BrainView, String> {
    let CreateBrainRequest {
        selection_token,
        name,
        role,
        routing_hints,
        egress_policy,
        write_policy,
        graph_enabled,
    } = request;
    let root = selected_folder(&state, &selection_token)?;
    let name = name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err("Give this brain a concise name (1–100 characters).".to_owned());
    }
    let (path, mut config) = load_or_create_config()?;
    if config.brains.iter().any(|brain| {
        fs::canonicalize(&brain.root_path)
            .ok()
            .is_some_and(|candidate| candidate == root)
    }) {
        return Err("That folder is already registered as a Kairos brain.".to_owned());
    }
    let existing_ids = config
        .brains
        .iter()
        .map(|brain| brain.id.clone())
        .collect::<HashSet<_>>();
    let (router_paths, context_paths) = known_brain_paths(&root);
    let mut read_policy = default_brain_read_policy();
    read_policy.startup_allow = router_paths
        .iter()
        .chain(context_paths.iter())
        .cloned()
        .collect();
    // A user-selected vault is allowed to be searched locally, while private
    // notes still remain withheld by the existing deny/explicit checks.
    read_policy.retrieval_allow = vec!["*.md".to_owned(), "**/*.md".to_owned()];
    let mut sanitized_hints = routing_hints
        .into_iter()
        .map(|hint| hint.trim().to_lowercase())
        .filter(|hint| (2..=80).contains(&hint.len()))
        .filter(|hint| hint.chars().all(|character| !character.is_control()))
        .collect::<Vec<_>>();
    sanitized_hints.sort();
    sanitized_hints.dedup();
    sanitized_hints.truncate(24);
    let write_policy = parse_write_policy(&write_policy)?;
    let brain = BrainRecord {
        id: slugify_brain_id(name, &existing_ids),
        name: name.to_owned(),
        role: parse_brain_role(&role),
        root_path: root,
        router_paths,
        context_paths,
        routing_hints: sanitized_hints,
        enabled: true,
        read_policy,
        egress_policy: parse_egress_policy(&egress_policy)?,
        write_policy: write_policy.clone(),
        write_directories: if write_policy == WritePolicy::ProposeConfirm {
            write_directories_for_root(&selected_folder(&state, &selection_token)?)
        } else {
            Vec::new()
        },
        graph_enabled,
        graph_include_patterns: vec!["**/*.md".to_owned()],
    };
    let view = brain_view(&brain);
    config.brains.push(brain);
    config.ensure_current_version();
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    state
        .folder_selections
        .lock()
        .map_err(|_| "Kairos folder-selection state is unavailable.".to_owned())?
        .remove(&selection_token);
    Ok(view)
}

/// Change only the explicit policies of an already registered brain. This is
/// intentionally separate from folder selection: a settings action cannot
/// broaden its root, router files, or read scope.
#[tauri::command]
fn update_brain_policy(
    brain_id: String,
    egress_policy: String,
    write_policy: String,
    graph_enabled: bool,
) -> Result<BrainView, String> {
    let (path, mut config) = load_or_create_config()?;
    let requested_egress = parse_egress_policy(&egress_policy)?;
    let requested_write = parse_write_policy(&write_policy)?;
    let brain = config
        .brains
        .iter_mut()
        .find(|brain| brain.id == brain_id)
        .ok_or_else(|| format!("Kairos cannot find brain `{brain_id}`."))?;

    let write_directories = if requested_write == WritePolicy::ProposeConfirm {
        let allowed = write_directories_for_root(&brain.root_path);
        if allowed.is_empty() {
            return Err(
                "Kairos found no approved Markdown directories in this brain. Select a narrower folder with a supported note directory before enabling writes."
                    .to_owned(),
            );
        }
        allowed
    } else {
        Vec::new()
    };

    brain.egress_policy = requested_egress;
    brain.write_policy = requested_write;
    brain.write_directories = write_directories;
    brain.graph_enabled = graph_enabled;
    let view = brain_view(brain);
    config.ensure_current_version();
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    Ok(view)
}

fn collect_graph_paths(brain: &BrainRecord) -> Vec<PathBuf> {
    let root = match fs::canonicalize(&brain.root_path) {
        Ok(root) => root,
        Err(_) => return Vec::new(),
    };
    let mut paths = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(directory) = stack.pop() {
        if paths.len() >= MAX_GRAPH_SCAN_FILES {
            break;
        }
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if paths.len() >= MAX_GRAPH_SCAN_FILES {
                break;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let relative = match path.strip_prefix(&root) {
                Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };
            if file_type.is_dir() {
                if !relative.split('/').any(|component| {
                    component.starts_with('.')
                        || component == "90_Private"
                        || component == "Attachments"
                }) {
                    stack.push(path);
                }
                continue;
            }
            if !file_type.is_file()
                || path.extension().and_then(|extension| extension.to_str()) != Some("md")
                || !graph_path_allowed(brain, &relative)
            {
                continue;
            }
            let Ok(canonical) = canonicalize_allowed_file(brain, &relative) else {
                continue;
            };
            let Ok(markdown) = fs::read_to_string(canonical) else {
                continue;
            };
            if evaluate_access(brain, &relative, &markdown, &[]).ok()
                == Some(AccessDisposition::Allowed)
            {
                paths.push(PathBuf::from(relative));
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn graph_path_allowed(brain: &BrainRecord, relative_path: &str) -> bool {
    let mut builder = GlobSetBuilder::new();
    let patterns = if brain.graph_include_patterns.is_empty() {
        vec!["**/*.md".to_owned()]
    } else {
        brain.graph_include_patterns.clone()
    };
    for pattern in patterns {
        let Ok(glob) = Glob::new(&pattern) else {
            continue;
        };
        builder.add(glob);
    }
    builder
        .build()
        .map(|set| set.is_match(relative_path))
        .unwrap_or(false)
}

fn graph_sources(config: &KairosConfig) -> Vec<GraphBrainSource> {
    config
        .brains
        .iter()
        .filter(|brain| brain.enabled && brain.graph_enabled)
        .map(|brain| GraphBrainSource {
            brain_id: brain.id.clone(),
            brain_name: brain.name.clone(),
            root_path: brain.root_path.clone(),
            allowlisted_markdown_paths: collect_graph_paths(brain),
            protected_relative_paths: vec![
                PathBuf::from("90_Private"),
                PathBuf::from("Attachments"),
                PathBuf::from(".obsidian"),
            ],
        })
        .collect()
}

fn graph_cache_path() -> Result<PathBuf, String> {
    Ok(kairos_core::application_support_dir()
        .map_err(|error| error.to_string())?
        .join("graph-index.json"))
}

fn persist_graph_cache(index: &GraphIndex) -> Result<(), String> {
    let path = graph_cache_path()?;
    let parent = path
        .parent()
        .ok_or_else(|| "graph cache has no parent directory".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| format!("could not create graph cache: {error}"))?;
    let temporary = path.with_extension("json.tmp");
    fs::write(
        &temporary,
        format!(
            "{}\n",
            serde_json::to_string_pretty(index)
                .map_err(|error| format!("could not encode graph cache: {error}"))?
        ),
    )
    .map_err(|error| format!("could not write graph cache: {error}"))?;
    fs::rename(temporary, path).map_err(|error| format!("could not finalise graph cache: {error}"))
}

#[tauri::command]
fn graph_snapshot() -> Result<GraphSnapshot, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let index = build_graph_index(&graph_sources(&config), GraphBuildOptions::default());
    persist_graph_cache(&index)?;
    let stale = !index.diagnostics.is_empty();
    Ok(GraphSnapshot {
        nodes: index.nodes,
        edges: index.edges,
        clusters: index.clusters,
        diagnostics: index.diagnostics,
        indexed_at: index.generated_at.to_rfc3339(),
        stale,
        capped: index.capped,
    })
}

#[tauri::command]
fn reveal_graph_node(node_id: String) -> Result<(), String> {
    if node_id.trim().is_empty() || node_id.len() > 512 {
        return Err("Choose a valid graph node first.".to_owned());
    }
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let index = build_graph_index(&graph_sources(&config), GraphBuildOptions::default());
    let node = index
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or_else(|| {
            "That graph node is no longer available; refresh the atlas first.".to_owned()
        })?;
    match node.kind {
        GraphNodeKind::Brain => {
            let brain_id = node
                .brain_id
                .as_deref()
                .ok_or_else(|| "This brain has no registered root.".to_owned())?;
            let brain = config
                .brains
                .iter()
                .find(|brain| brain.id == brain_id && brain.enabled && brain.graph_enabled)
                .ok_or_else(|| "This graph brain is no longer enabled.".to_owned())?;
            let root = fs::canonicalize(&brain.root_path)
                .map_err(|_| "This brain root is currently unavailable.".to_owned())?;
            system::open_in_finder(&root)
        }
        GraphNodeKind::Note | GraphNodeKind::Bridge => {
            let brain_id = node
                .brain_id
                .as_deref()
                .ok_or_else(|| "This graph note has no owning brain.".to_owned())?;
            let relative_path = node
                .relative_path
                .as_deref()
                .ok_or_else(|| "This graph note has no safe source path.".to_owned())?;
            let brain = config
                .brains
                .iter()
                .find(|brain| brain.id == brain_id && brain.enabled && brain.graph_enabled)
                .ok_or_else(|| "This graph brain is no longer enabled.".to_owned())?;
            if !graph_path_allowed(brain, relative_path) {
                return Err("This graph note is no longer permitted for inspection.".to_owned());
            }
            let canonical = canonicalize_allowed_file(brain, relative_path)
                .map_err(|error| error.to_string())?;
            let markdown = fs::read_to_string(&canonical)
                .map_err(|_| "This graph note is unavailable.".to_owned())?;
            if evaluate_access(brain, relative_path, &markdown, &[])
                .map_err(|error| error.to_string())?
                != AccessDisposition::Allowed
            {
                return Err("This graph note is no longer permitted for inspection.".to_owned());
            }
            system::reveal_in_finder(&canonical)
        }
        GraphNodeKind::Tag => Err(
            "A tag is metadata, not a file. Select one of its connected notes to reveal it."
                .to_owned(),
        ),
        GraphNodeKind::Kairos => {
            Err("Kairos is a virtual graph node and has no Finder location.".to_owned())
        }
    }
}

#[tauri::command]
fn draft_note_write(
    state: State<'_, AppState>,
    brain_id: String,
    relative_path: String,
    kind: NoteWriteKind,
    markdown: String,
) -> Result<NoteWriteProposal, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let brain = config
        .brains
        .iter()
        .find(|brain| brain.id == brain_id)
        .ok_or_else(|| format!("Kairos cannot find brain `{brain_id}`."))?;
    let options = WriteProposalOptions::for_brain(brain);
    state
        .write_proposals
        .lock()
        .map_err(|_| "Kairos write-proposal state is unavailable.".to_owned())?
        .propose(
            brain,
            NoteWriteRequest {
                brain_id,
                relative_path,
                kind,
                markdown,
            },
            &options,
            &[],
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn confirm_note_write(
    state: State<'_, AppState>,
    proposal_id: String,
    nonce: String,
) -> Result<ConfirmedNoteWrite, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let mut store = state
        .write_proposals
        .lock()
        .map_err(|_| "Kairos write-proposal state is unavailable.".to_owned())?;
    // The store reveals only the proposal's registered brain ID, never its
    // Markdown. That lets us re-authorize the exact policy record and preserve
    // a useful conflict/expiry error for the confirmation screen.
    let brain_id = store
        .proposal_brain_id(&proposal_id)
        .ok_or_else(|| "This write proposal is missing or expired.".to_owned())?;
    let brain = config
        .brains
        .iter()
        .find(|brain| brain.id == brain_id)
        .ok_or_else(|| "This write proposal's registered brain is unavailable.".to_owned())?;
    let options = WriteProposalOptions::for_brain(brain);
    store
        .confirm(
            brain,
            &NoteWriteConfirmation { proposal_id, nonce },
            &options,
            &[],
        )
        .map_err(|error| error.to_string())
}

fn configured_shortcut() -> Shortcut {
    let configured = config_path()
        .ok()
        .filter(|path| path.exists())
        .and_then(|path| load_app_config(&path).ok())
        .map(|config| config.app.summon_shortcut)
        .unwrap_or_else(|| "Alt+Space".to_owned());
    normalise_shortcut(&configured)
        .map(|(_, shortcut)| shortcut)
        .unwrap_or_else(|_| Shortcut::from_str("Alt+Space").expect("static shortcut"))
}

fn toggle_window(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if window.is_visible()? {
        window.hide()?;
    } else {
        window.show()?;
        window.set_focus()?;
        let target = config_path()
            .ok()
            .filter(|path| path.exists())
            .and_then(|path| load_app_config(&path).ok())
            .map(|config| config.app.summon_target)
            .unwrap_or(SummonTarget::CompactChat);
        let _ = app.emit("kairos://summon", target);
    }
    Ok(())
}

fn close_should_hide() -> bool {
    config_path()
        .ok()
        .filter(|path| path.exists())
        .and_then(|path| load_app_config(&path).ok())
        .map(|config| config.app.close_to_hide)
        .unwrap_or(true)
}

pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_window(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let (Some(window), Ok(path)) = (app.get_webview_window("main"), config_path()) {
                if let Ok(config) = load_app_config(&path) {
                    let _ = window.set_always_on_top(config.app.keep_above_other_windows);
                }
            }

            app.global_shortcut().register(configured_shortcut())?;
            let show_item = MenuItem::with_id(app, "show", "Show Kairos", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Kairos", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
            let template_icon =
                Image::from_bytes(include_bytes!("../icons/kairos-template-18.png"))?;
            TrayIconBuilder::with_id("kairos-menu")
                .icon(template_icon)
                .icon_as_template(true)
                .tooltip("Kairos — the right moment")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        let _ = toggle_window(app);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let _ = toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && close_should_hide()
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            local_setup_status,
            set_memory_budget,
            set_hardware_profile,
            initialize_tharm_profile,
            set_selected_model,
            pull_ollama_model,
            cancel_ollama_pull,
            test_ollama_model,
            open_ollama_install_page,
            set_surface_mode,
            pick_temp_attachments,
            discard_temp_attachment,
            brief_context,
            brief_with_ollama,
            chat_preview,
            chat_with_provider,
            grant_scoped_execution,
            list_chat_sessions,
            load_chat_session,
            create_chat_session,
            rename_chat_session,
            delete_chat_session,
            clear_chat_history,
            save_provider_settings,
            save_app_preferences,
            list_brains,
            pick_brain_folder,
            inspect_brain_folder,
            create_brain,
            update_brain_policy,
            graph_snapshot,
            reveal_graph_node,
            draft_note_write,
            confirm_note_write
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kairos");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apple_capability(memory_gb: u16) -> system::MachineCapability {
        system::MachineCapability {
            architecture: "aarch64".to_owned(),
            detected_hardware_profile: HardwareProfile::AppleUnified,
            unified_memory_bytes: Some(memory_gb as u64 * 1024_u64.pow(3)),
            unified_memory_gib: Some(memory_gb),
            nvidia_vram_gib: None,
            free_disk_bytes: None,
            primary_fit_capacity_gib: Some(memory_gb),
            detected_memory_budget_gib: Some(memory_gb),
        }
    }

    #[test]
    fn auto_profile_caps_a_large_preference_at_detected_apple_memory() {
        let mut setup = LocalSetupSettings::default();
        setup
            .set_memory_budget(MemoryBudgetMode::Preset, Some(96))
            .unwrap();
        let local_model = LocalModelSettings::default();

        let plan = resolve_hardware_plan(&setup, &apple_capability(48));

        assert_eq!(plan.effective_profile, HardwareProfile::AppleUnified);
        assert_eq!(plan.primary_fit_limit_gb, Some(48));
        assert!(!plan.planning_override);
        assert_eq!(local_model.selected_model, kairos_core::DEFAULT_LOCAL_MODEL);
        assert_eq!(
            local_model.context_window_tokens,
            DEFAULT_CONTEXT_WINDOW_TOKENS
        );
    }

    #[test]
    fn manual_nvidia_profile_is_a_planning_override_not_detected_hardware() {
        let mut setup = LocalSetupSettings::default();
        setup.set_hardware_profile(HardwareProfile::NvidiaVram);
        setup
            .set_memory_budget(MemoryBudgetMode::Preset, Some(24))
            .unwrap();

        let plan = resolve_hardware_plan(&setup, &apple_capability(48));

        assert_eq!(plan.detected_profile, HardwareProfile::AppleUnified);
        assert_eq!(plan.effective_profile, HardwareProfile::NvidiaVram);
        assert_eq!(plan.primary_fit_limit_gb, Some(24));
        assert!(plan.planning_override);
    }
}
