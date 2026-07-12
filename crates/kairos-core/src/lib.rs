//! Policy-first routing and bounded context packing for Kairos.
//!
//! This crate deliberately knows nothing about a model provider or a WebView.
//! It is the single place where connected-brain paths and privacy policy are
//! enforced before any content leaves the local filesystem boundary.

mod config;
mod context;
mod error;
mod policy;
mod provider;
mod routing;

pub use config::{
    BrainRecord, BrainRole, DEFAULT_CONTEXT_WINDOW_TOKENS, DEFAULT_LOCAL_MODEL,
    DEFAULT_OLLAMA_ENDPOINT, EgressPolicy, FAST_ROUTER_MODEL, KairosConfig, LocalModelChoice,
    LocalModelSettings, OPTIONAL_LOCAL_MODELS, ReadPolicy, WritePolicy, application_support_dir,
    default_config_path, default_tharm_config, load_config, local_model_choices, write_config,
};
pub use context::{
    ContextPack, DeniedSource, SourceExcerpt, SourceRef, build_context, render_handoff,
};
pub use error::{CoreError, Result};
pub use policy::{
    AccessDisposition, AccessGrant, ContentDestination, canonicalize_allowed_file,
    enforce_content_egress, evaluate_access, preflight_retrieval_access, preflight_startup_access,
};
pub use provider::{BriefAnswer, OllamaProvider, OllamaStatus, ollama_status, synthesize_ollama};
pub use routing::{RouteResult, RoutedBrain, route_query};
