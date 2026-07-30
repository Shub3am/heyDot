mod commands;
mod local_model;

use std::sync::Arc;
use std::time::Duration;

use dot_models::{detect_hardware, recommend};
use tauri::{Manager, RunEvent};

use crate::local_model::{LocalModel, LocalModelPaths, pick_local_model};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let http = reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(60))
                .build()?;
            let log_dir = app.path().home_dir()?.join("Library/Logs/Hey Dot");
            std::fs::create_dir_all(&log_dir)?;
            let paths = LocalModelPaths {
                models_dir: app.path().data_dir()?.join("Hey Dot/models"),
                // Tauri copies externalBin next to the app's own executable, without the target triple.
                server_binary: std::env::current_exe()?.with_file_name("llama-server"),
                log_file: log_dir.join("llama-server.log"),
            };
            let model = pick_local_model(&recommend(&detect_hardware()));
            let local_model = Arc::new(LocalModel::new(model, paths, http.clone()));
            app.manage(http);
            app.manage(Arc::clone(&local_model));
            tauri::async_runtime::spawn(async move { local_model.start_if_installed().await });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::watch_local_model,
            commands::download_local_model,
            commands::ask_text
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Tauri ends with process::exit, which skips destructors, so kill_on_drop never fires.
            if let RunEvent::Exit = event {
                let local_model = app.state::<Arc<LocalModel>>();
                tauri::async_runtime::block_on(local_model.shutdown());
            }
        });
}
