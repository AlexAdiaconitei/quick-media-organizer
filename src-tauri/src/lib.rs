// MSVC prints an informational line while creating the import library for the
// cdylib. It is not a problem in this crate and only adds noise to every build.
#![allow(unknown_lints)]
#![allow(linker_messages)]

mod batch;
mod commands;
mod error_log;
mod fs_util;
mod media;
mod models;
mod path_util;
mod rename;
mod session;
mod state;
mod video;

use batch::{BatchRunner, SharedBatchState};
use error_log::{ErrorLog, SharedErrorLog};
use state::{AppState, SharedState};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            let error_log = ErrorLog::new(app_data_dir.clone());
            app.manage(SharedErrorLog::new(error_log));
            app.manage(SharedState::new(AppState::new(app_data_dir)));
            app.manage(SharedBatchState::new(BatchRunner::new()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_settings,
            commands::complete_first_run,
            commands::set_locale,
            commands::set_ui_preferences,
            commands::pick_folder,
            commands::open_folder,
            commands::get_state,
            commands::rename_current,
            commands::trash_current,
            commands::move_current_to_folder,
            commands::skip_current,
            commands::dismiss_session_complete,
            commands::restart_queue,
            commands::undo_last,
            commands::set_armed_folder,
            commands::toggle_favorite_folder,
            commands::set_options,
            commands::check_ffmpeg,
            commands::resolve_video_preview,
            commands::diagnose_media_file,
            commands::trim_current_video,
            commands::report_error,
            commands::get_error_log,
            commands::get_error_log_path,
            commands::clear_error_log,
            commands::list_queue_items,
            commands::describe_media_paths,
            commands::pick_media_files,
            commands::get_ffmpeg_capabilities,
            commands::get_batch_presets,
            commands::save_batch_preset,
            commands::delete_batch_preset,
            commands::get_last_batch_settings,
            commands::start_batch_job,
            commands::cancel_batch_job,
            commands::get_batch_job,
            commands::finalize_batch_job,
            commands::probe_video_duration,
            commands::scan_folder_media,
            commands::get_active_batch_job,
            commands::get_update_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
