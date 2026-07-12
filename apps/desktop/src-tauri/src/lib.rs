use serde::Serialize;
use tauri::Manager;
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

use kairos_core::{
    BriefAnswer, ContentDestination, ContextPack, LocalModelChoice, LocalModelSettings,
    build_context, default_config_path, default_tharm_config, enforce_content_egress, load_config,
    local_model_choices, ollama_status, synthesize_ollama, write_config,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    config_path: String,
    initialized: bool,
    model: ModelStatus,
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalBrief {
    answer: BriefAnswer,
    context: ContextPack,
}

fn config_path() -> Result<std::path::PathBuf, String> {
    default_config_path().map_err(|error| error.to_string())
}

fn load_app_config(path: &std::path::Path) -> Result<kairos_core::KairosConfig, String> {
    let mut config = load_config(path).map_err(|error| error.to_string())?;
    if config.version < 2 {
        config.version = 2;
        write_config(path, &config, true).map_err(|error| error.to_string())?;
    }
    Ok(config)
}

async fn model_status(settings: &LocalModelSettings) -> ModelStatus {
    let readiness = ollama_status(settings).await;
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
    }
}

async fn status() -> Result<AppStatus, String> {
    let config_path = config_path()?;
    let initialized = config_path.exists();
    let settings = if initialized {
        load_app_config(&config_path)?.local_model
    } else {
        LocalModelSettings::default()
    };
    Ok(AppStatus {
        config_path: config_path.display().to_string(),
        initialized,
        model: model_status(&settings).await,
    })
}

#[tauri::command]
async fn app_status() -> Result<AppStatus, String> {
    status().await
}

#[tauri::command]
async fn initialize_tharm_profile() -> Result<AppStatus, String> {
    let path = config_path()?;
    let config = default_tharm_config().map_err(|error| error.to_string())?;
    write_config(&path, &config, false).map_err(|error| error.to_string())?;
    status().await
}

#[tauri::command]
async fn set_selected_model(model: String) -> Result<AppStatus, String> {
    let path = config_path()?;
    let mut config = load_app_config(&path)?;
    config
        .local_model
        .set_selected_model(&model)
        .map_err(|error| error.to_string())?;
    write_config(&path, &config, true).map_err(|error| error.to_string())?;
    status().await
}

#[tauri::command]
fn brief_context() -> Result<kairos_core::ContextPack, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    build_context(&config, "What should I do next, and why?", None, &[], None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn brief_with_ollama() -> Result<LocalBrief, String> {
    let path = config_path()?;
    let config = load_app_config(&path)?;
    let context = build_context(&config, "What should I do next, and why?", None, &[], None)
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

fn toggle_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if window.is_visible()? {
        window.hide()?;
    } else {
        window.show()?;
        window.set_focus()?;
    }
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let _ = toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
            app.global_shortcut().register(shortcut)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            initialize_tharm_profile,
            set_selected_model,
            brief_context,
            brief_with_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kairos");
}
