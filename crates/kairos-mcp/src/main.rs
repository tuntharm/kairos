use std::path::PathBuf;

use clap::Parser;
use kairos_core::{
    ContentDestination, RouteResult, default_config_path, enforce_content_egress,
    load_or_migrate_config, route_query,
};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(name = "kairos-mcp", about = "Read-only local Kairos MCP server")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Clone)]
struct KairosServer {
    config_path: PathBuf,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct QueryInput {
    #[schemars(description = "The user question to route through Kairos")]
    query: String,
    #[schemars(description = "Optional enabled brain ID selected explicitly by the user")]
    brain_override: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HandoffInput {
    #[schemars(description = "Target agent or workspace, for example Codex or Cursor")]
    target: String,
    #[schemars(description = "Concrete task the receiving agent should perform")]
    task: String,
    #[schemars(description = "Question whose approved sources should form the handoff")]
    query: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExternalContextNotice {
    route: RouteResult,
    content_available: bool,
    message: String,
}

impl KairosServer {
    fn new(config_path: PathBuf) -> Self {
        Self {
            config_path,
            tool_router: Self::tool_router(),
        }
    }

    fn config(&self) -> Result<kairos_core::KairosConfig, String> {
        load_or_migrate_config(&self.config_path).map_err(|error| error.to_string())
    }

    fn json<T: serde::Serialize>(value: T) -> String {
        serde_json::to_string_pretty(&value)
            .unwrap_or_else(|error| format!("{{\"error\":\"{error}\"}}"))
    }

    fn external_context_notice(
        &self,
        query: &str,
        brain_override: Option<&str>,
    ) -> Result<ExternalContextNotice, String> {
        let config = self.config()?;
        let route =
            route_query(&config, query, brain_override).map_err(|error| error.to_string())?;
        let policy_reason =
            enforce_content_egress(&config, &route, ContentDestination::ExternalMcp, false)
                .err()
                .map(|error| error.to_string());
        Ok(ExternalContextNotice {
            route,
            content_available: false,
            message: policy_reason.unwrap_or_else(|| {
                "MCP source-content egress is disabled in this Kairos alpha. Use the local desktop app or CLI for synthesis."
                    .to_owned()
            }),
        })
    }
}

#[tool_router]
impl KairosServer {
    #[tool(
        description = "Route a question to at most two enabled Kairos brains without reading arbitrary vault contents."
    )]
    fn kairos_route(&self, Parameters(input): Parameters<QueryInput>) -> String {
        match self.config().and_then(|config| {
            route_query(&config, &input.query, input.brain_override.as_deref())
                .map_err(|error| error.to_string())
        }) {
            Ok(route) => Self::json(route),
            Err(error) => format!("Kairos route error: {error}"),
        }
    }

    #[tool(
        description = "Return safe routing metadata only. This alpha never sends note contents through MCP; use the local Kairos desktop app or CLI for synthesis."
    )]
    fn kairos_context(&self, Parameters(input): Parameters<QueryInput>) -> String {
        match self.external_context_notice(&input.query, input.brain_override.as_deref()) {
            Ok(notice) => Self::json(notice),
            Err(error) => format!("Kairos context error: {error}"),
        }
    }

    #[tool(
        description = "Return daily-brief routing metadata only. Note contents remain inside local Kairos in this alpha."
    )]
    fn kairos_brief(&self) -> String {
        match self.external_context_notice("What should I do next, and why?", None) {
            Ok(notice) => Self::json(notice),
            Err(error) => format!("Kairos brief error: {error}"),
        }
    }

    #[tool(
        description = "Create a copyable, route-only handoff packet without writing into any brain or chat log. Note text is deliberately omitted."
    )]
    fn kairos_handoff(&self, Parameters(input): Parameters<HandoffInput>) -> String {
        match self.external_context_notice(&input.query, None) {
            Ok(notice) => format!(
                "# [Kairos -> {}] {}\n\n\
                 ## Routing\n\
                 Query: {}\n\n\
                 Routed brains: {}\n\n\
                 ## Local-only boundary\n\
                 Note text and source excerpts were intentionally not transferred through MCP. {}\n",
                input.target,
                input.task,
                notice.route.query,
                notice
                    .route
                    .brains
                    .iter()
                    .map(|brain| brain.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                notice.message,
            ),
            Err(error) => format!("Kairos handoff error: {error}"),
        }
    }
}

#[tool_handler]
impl ServerHandler for KairosServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Kairos is a read-only, route-only local brain router in this alpha. MCP never receives note content; use the local desktop app or CLI for synthesis."
                    .to_owned(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let server = KairosServer::new(args.config.unwrap_or(default_config_path()?));
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
