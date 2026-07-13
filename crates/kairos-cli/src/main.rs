use std::path::PathBuf;

use clap::{Parser, Subcommand};
use kairos_core::{
    ContentDestination, ContextPack, build_context, default_config_path, default_user_config,
    enforce_content_egress, load_or_migrate_config, ollama_status, render_handoff, route_query,
    synthesize_ollama, write_config,
};
use serde_json::{Value, json};

#[derive(Parser, Debug)]
#[command(name = "kairos", about = "Local-first brain composer")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Create an empty local Kairos registry. Add brains through the desktop app.
    Init {
        #[arg(long)]
        force: bool,
    },
    /// Check local brain paths and the Ollama loopback endpoint.
    Doctor,
    /// Show or persist the selected local Ollama model.
    Model {
        /// A model identifier, for example qwen3:8b. Omit to inspect the saved selection.
        model: Option<String>,
    },
    /// Recommend a brain without reading arbitrary notes.
    Route {
        query: String,
        #[arg(long)]
        brain: Option<String>,
    },
    /// Emit a bounded, cited context pack.
    Context {
        query: String,
        #[arg(long)]
        brain: Option<String>,
    },
    /// Build the daily next-action context and optionally synthesize it locally.
    Brief {
        #[arg(long)]
        offline: bool,
    },
    /// Render a copyable handoff without writing into a brain.
    Handoff {
        target: String,
        task: String,
        #[arg(long, default_value = "What should I do next, and why?")]
        query: String,
    },
}

fn config_path(cli: &Cli) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(cli.config.clone().unwrap_or(default_config_path()?))
}

fn load(cli: &Cli) -> Result<kairos_core::KairosConfig, Box<dyn std::error::Error>> {
    Ok(load_or_migrate_config(config_path(cli)?)?)
}

fn print_json(value: &impl serde::Serialize) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn context_for_brief(
    config: &kairos_core::KairosConfig,
) -> Result<ContextPack, Box<dyn std::error::Error>> {
    Ok(build_context(
        config,
        "What should I do next, and why?",
        None,
        &[],
        None,
    )?)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match &cli.command {
        Command::Init { force } => {
            let path = config_path(&cli)?;
            let config = default_user_config()?;
            write_config(&path, &config, *force)?;
            println!("Initialized an empty Kairos registry at {}", path.display());
        }
        Command::Doctor => {
            let path = config_path(&cli)?;
            let config = load(&cli)?;
            let brain_status: Vec<Value> = config
                .brains
                .iter()
                .map(|brain| {
                    json!({
                        "id": brain.id,
                        "enabled": brain.enabled,
                        "rootExists": brain.root_path.exists(),
                        "egressPolicy": brain.egress_policy,
                    })
                })
                .collect();
            let local_model = ollama_status(&config.local_model).await;
            print_json(&json!({
                "configPath": path,
                "sourceRouterExists": config.source_router_path.exists(),
                "brains": brain_status,
                "localModel": local_model,
            }))?;
        }
        Command::Model { model } => {
            let path = config_path(&cli)?;
            let mut config = load(&cli)?;
            if let Some(model) = model {
                config.local_model.set_selected_model(model)?;
                config.ensure_current_version();
                write_config(&path, &config, true)?;
            }
            print_json(&json!({
                "localModel": ollama_status(&config.local_model).await,
            }))?;
        }
        Command::Route { query, brain } => {
            print_json(&route_query(&load(&cli)?, query, brain.as_deref())?)?;
        }
        Command::Context { query, brain } => {
            print_json(&build_context(
                &load(&cli)?,
                query,
                brain.as_deref(),
                &[],
                None,
            )?)?;
        }
        Command::Brief { offline } => {
            let config = load(&cli)?;
            let pack = context_for_brief(&config)?;
            if *offline {
                print_json(&pack)?;
            } else {
                enforce_content_egress(
                    &config,
                    &pack.route,
                    ContentDestination::LocalOllama,
                    false,
                )?;
                let answer = synthesize_ollama(&config.local_model, &pack).await?;
                let sources = pack
                    .sources
                    .iter()
                    .map(|excerpt| &excerpt.source)
                    .collect::<Vec<_>>();
                print_json(&json!({
                    "answer": answer,
                    "sources": sources,
                    "freshnessWarnings": pack.freshness_warnings,
                }))?
            }
        }
        Command::Handoff {
            target,
            task,
            query,
        } => {
            let pack = build_context(&load(&cli)?, query, None, &[], None)?;
            print!("{}", render_handoff(&pack, target, task));
        }
    }
    Ok(())
}
