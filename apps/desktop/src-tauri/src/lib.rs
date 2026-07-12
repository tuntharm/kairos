use serde::Serialize;
use tauri::Manager;
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};

use kairos_core::{
    BriefAnswer, ContentDestination, ContextPack, build_context, default_config_path,
    default_tharm_config, enforce_content_egress, load_config, ollama_reachable, synthesize_ollama,
    write_config,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    config_path: String,
    initialized: bool,
    ollama_reachable: bool,
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

async fn status() -> Result<AppStatus, String> {
    let config_path = config_path()?;
    let initialized = config_path.exists();
    Ok(AppStatus {
        config_path: config_path.display().to_string(),
        initialized,
        ollama_reachable: ollama_reachable().await,
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
fn brief_context() -> Result<kairos_core::ContextPack, String> {
    let config = load_config(config_path()?).map_err(|error| error.to_string())?;
    build_context(&config, "What should I do next, and why?", None, &[], None)
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn brief_with_ollama() -> Result<LocalBrief, String> {
    let config = load_config(config_path()?).map_err(|error| error.to_string())?;
    let context = build_context(&config, "What should I do next, and why?", None, &[], None)
        .map_err(|error| error.to_string())?;
    enforce_content_egress(
        &config,
        &context.route,
        ContentDestination::LocalOllama,
        false,
    )
    .map_err(|error| error.to_string())?;
    let answer = synthesize_ollama("qwen3:8b", &context)
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
            brief_context,
            brief_with_ollama
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kairos");
}
