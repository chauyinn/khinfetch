mod commands;
pub mod domain;
pub mod downloads;
pub mod khinsider;
pub mod logging;
pub mod storage;

use tauri::Manager;

use commands::{
    cancel_job, choose_download_directory, enqueue_album, enqueue_urls, get_album_details,
    get_settings, get_track_preview, get_update_environment, list_jobs, open_download_directory,
    open_external_url, open_log_directory, pause_job, remove_job, resume_job, retry_job,
    search_albums, update_settings, AppState,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| std::io::Error::other(format!("无法定位应用数据目录: {error}")))?;
            std::fs::create_dir_all(&data_dir)?;
            let database_path = data_dir.join("khinfetch.sqlite3");
            let mut state = tauri::async_runtime::block_on(AppState::new(database_path))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            let logging = logging::initialize(&data_dir.join("logs"))?;
            state.install_logging(logging);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "khinfetch started");
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_albums,
            get_album_details,
            get_track_preview,
            list_jobs,
            get_settings,
            get_update_environment,
            update_settings,
            enqueue_album,
            enqueue_urls,
            pause_job,
            resume_job,
            cancel_job,
            retry_job,
            remove_job,
            choose_download_directory,
            open_download_directory,
            open_log_directory,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running khinfetch");
}
