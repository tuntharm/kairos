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
    BrainRecord, BrainRole, EgressPolicy, KairosConfig, ReadPolicy, WritePolicy,
    application_support_dir, default_config_path, default_tharm_config, load_config, write_config,
};
pub use context::{
    ContextPack, DeniedSource, SourceExcerpt, SourceRef, build_context, render_handoff,
};
pub use error::{CoreError, Result};
pub use policy::{
    AccessDisposition, AccessGrant, ContentDestination, canonicalize_allowed_file,
    enforce_content_egress, evaluate_access, preflight_startup_access,
};
pub use provider::{BriefAnswer, ollama_reachable, synthesize_ollama};
pub use routing::{RouteResult, RoutedBrain, route_query};
