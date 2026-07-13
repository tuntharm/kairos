//! Policy-first routing and bounded context packing for Kairos.
//!
//! This crate deliberately knows nothing about a model provider or a WebView.
//! It is the single place where connected-brain paths and privacy policy are
//! enforced before any content leaves the local filesystem boundary.

mod config;
mod context;
mod error;
mod graph;
mod policy;
mod provider;
mod routing;
mod write;

pub use config::{
    AppSettings, BrainRecord, BrainRole, CURRENT_CONFIG_VERSION, DEFAULT_CONTEXT_WINDOW_TOKENS,
    DEFAULT_LOCAL_MODEL, DEFAULT_OLLAMA_ENDPOINT, EgressPolicy, FAST_ROUTER_MODEL, HardwareProfile,
    InferenceSettings, KairosConfig, LocalModelArtifact, LocalModelChoice, LocalModelFit,
    LocalModelFitAssessment, LocalModelProfile, LocalModelRecommendation, LocalModelSettings,
    LocalModelVerification, LocalSetupSettings, MEMORY_BUDGET_PRESETS_GB, MemoryBudgetMode,
    OPTIONAL_LOCAL_MODELS, ProviderConfig, ProviderKind, ReadPolicy, SummonTarget, WritePolicy,
    application_support_dir, assess_local_model_fit, assess_local_model_fit_for_hardware,
    default_brain_read_policy, default_config_path, default_user_config, load_config,
    load_or_migrate_config, local_model_choices, local_model_profile, local_model_profiles,
    local_model_recommendations, migration_backup_path, write_config,
};
pub use context::{
    ContextPack, DeniedSource, SourceExcerpt, SourceRef, build_context, render_handoff,
};
pub use error::{CoreError, Result};
pub use graph::{
    DEFAULT_GRAPH_NODE_CAP, ExplicitReference, ExplicitReferences, GraphBrainSource,
    GraphBuildOptions, GraphCluster, GraphClusterStatus, GraphDiagnostic, GraphDiagnosticKind,
    GraphEdge, GraphEdgeKind, GraphIndex, GraphNode, GraphNodeKind, HARD_GRAPH_NODE_CAP,
    KAIROS_NODE_ID, build_graph_index, extract_explicit_references, graph_freshness,
};
pub use policy::{
    AccessDisposition, AccessGrant, ContentDestination, canonicalize_allowed_file,
    enforce_content_egress, evaluate_access, preflight_retrieval_access, preflight_startup_access,
};
pub use provider::{
    BriefAnswer, ChatAnswer, ConversationTurn, InstalledModel, OllamaModelTest, OllamaProvider,
    OllamaPullProgress, OllamaStatus, chat_with_anthropic_api, chat_with_claude_cli,
    chat_with_codex_cli, chat_with_ollama, chat_with_openai_api, ollama_status, pull_ollama_model,
    render_chat_prompt, stream_chat_with_ollama, synthesize_ollama, test_ollama_model,
};
pub use routing::{CROSS_BRAIN_PULSE_QUERY, RouteResult, RoutedBrain, route_query};
pub use write::{
    ConfirmedNoteWrite, NoteWriteConfirmation, NoteWriteKind, NoteWriteProposal, NoteWriteRequest,
    WriteProposalOptions, WriteProposalStore, markdown_sha256,
};
