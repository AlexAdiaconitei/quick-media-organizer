use crate::batch::checkpoint::{BatchCheckpoint, BatchCheckpointStore};
use crate::batch::runner::{
    new_job_status, plan_items_with_capabilities, BatchEvents, FfmpegEncoder, MediaEncoder,
    SharedBatchState,
};
use crate::batch::video_backend::resolve_video_encoding;
use crate::batch::{
    BatchEstimate, BatchItemStatus, BatchJobStatus, BatchMediaType, BatchPreset,
    BatchProgressSummary, BatchSettings, FfmpegCapabilities, ImageFormat, OutputMode,
};
use crate::error_log::{ErrorEntry, SharedErrorLog};
use crate::models::{
    ActionResult, AppSettings, FrontendState, LayoutMode, MediaItem, RenameMode, SortMode,
};
use crate::state::SharedState;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};

fn log_rust_error(log: &SharedErrorLog, command: &str, error: &str) {
    if let Ok(guard) = log.lock() {
        let _ = guard.report(
            "rust",
            "error",
            error,
            Some(json!({ "command": command })),
            None,
        );
    }
}

fn wrap<T>(log: &SharedErrorLog, command: &str, result: Result<T, String>) -> Result<T, String> {
    if let Err(ref error) = result {
        log_rust_error(log, command, error);
    }
    result
}

#[tauri::command]
pub fn report_error(
    log: State<'_, SharedErrorLog>,
    source: String,
    message: String,
    context: Option<serde_json::Value>,
    stack: Option<String>,
) -> Result<ErrorEntry, String> {
    log.lock()
        .map_err(|e| e.to_string())?
        .report(&source, "error", &message, context, stack)
}

#[tauri::command]
pub fn get_error_log(log: State<'_, SharedErrorLog>) -> Result<Vec<ErrorEntry>, String> {
    log.lock().map_err(|e| e.to_string())?.list()
}

#[tauri::command]
pub fn get_error_log_path(log: State<'_, SharedErrorLog>) -> Result<String, String> {
    Ok(log.lock().map_err(|e| e.to_string())?.log_path())
}

#[tauri::command]
pub fn clear_error_log(log: State<'_, SharedErrorLog>) -> Result<(), String> {
    log.lock().map_err(|e| e.to_string())?.clear()
}

#[tauri::command]
pub fn get_app_settings(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
) -> Result<AppSettings, String> {
    wrap(
        &log,
        "get_app_settings",
        state
            .lock()
            .map_err(|e| e.to_string())
            .map(|g| g.app_settings.clone()),
    )
}

#[tauri::command]
pub fn complete_first_run(state: State<'_, SharedState>) -> Result<(), String> {
    state
        .lock()
        .map_err(|e| e.to_string())?
        .complete_first_run()
}

#[tauri::command]
pub fn set_locale(state: State<'_, SharedState>, locale: String) -> Result<(), String> {
    state.lock().map_err(|e| e.to_string())?.set_locale(locale)
}

#[tauri::command]
pub fn set_ui_preferences(
    state: State<'_, SharedState>,
    layout_mode: LayoutMode,
    show_metadata: bool,
    video_with_sound: bool,
) -> Result<AppSettings, String> {
    state.lock().map_err(|e| e.to_string())?.set_ui_preferences(
        layout_mode,
        show_metadata,
        video_with_sound,
    )
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let folder = app.dialog().file().blocking_pick_folder();

    Ok(folder.map(|p| p.to_string()))
}

#[tauri::command]
pub fn open_folder(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    path: String,
) -> Result<FrontendState, String> {
    wrap(
        &log,
        "open_folder",
        (|| {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.open_folder(PathBuf::from(path))?;
            let (session_reset, resume_from, subfolder_media_count) =
                guard.take_transient_open_notices();
            let mut frontend = guard.to_frontend_state();
            frontend.session_reset = session_reset;
            frontend.resume_from = resume_from;
            frontend.subfolder_media_count = subfolder_media_count;
            Ok(frontend)
        })(),
    )
}

#[tauri::command]
pub fn get_state(state: State<'_, SharedState>) -> Result<FrontendState, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.to_frontend_state())
}

#[tauri::command]
pub fn rename_current(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    name: String,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "rename_current",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .rename_current(&name),
    )
}

#[tauri::command]
pub fn trash_current(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "trash_current",
        state.lock().map_err(|e| e.to_string())?.trash_current(),
    )
}

#[tauri::command]
pub fn move_current_to_folder(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    folder: String,
    name: Option<String>,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "move_current_to_folder",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .move_current_to_folder(Some(folder), name),
    )
}

#[tauri::command]
pub fn skip_current(state: State<'_, SharedState>, delta: i32) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.skip(delta)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn dismiss_session_complete(state: State<'_, SharedState>) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.dismiss_session_complete()?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn restart_queue(state: State<'_, SharedState>) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.restart_queue()?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn undo_last(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "undo_last",
        state.lock().map_err(|e| e.to_string())?.undo_last(),
    )
}

#[tauri::command]
pub fn set_armed_folder(
    state: State<'_, SharedState>,
    folder: Option<String>,
) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.set_armed_folder(folder)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn toggle_favorite_folder(
    state: State<'_, SharedState>,
    folder: String,
) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.toggle_favorite(&folder)?;
    Ok(guard.to_frontend_state())
}

#[tauri::command]
pub fn set_options(
    state: State<'_, SharedState>,
    sort_mode: SortMode,
    scan_recursive: bool,
    rename_mode: RenameMode,
) -> Result<FrontendState, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.set_options(sort_mode, scan_recursive, rename_mode)?;
    Ok(guard.to_frontend_state())
}

/// Building a proxy runs ffmpeg, which takes seconds on a large clip, so the
/// app-state mutex is released before that starts: holding it would freeze
/// every other command for the duration.
#[tauri::command]
pub fn resolve_video_preview(
    state: State<'_, SharedState>,
    path: String,
) -> Result<crate::models::VideoPreviewInfo, String> {
    let cache_dir = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_data_dir.join("video-previews")
    };
    Ok(crate::video::resolve_video_preview(
        Path::new(&path),
        &cache_dir,
    ))
}

#[tauri::command]
pub fn diagnose_media_file(path: String) -> Result<crate::models::MediaFileDiagnosis, String> {
    Ok(crate::media::diagnose_media_file(Path::new(&path)))
}

#[tauri::command]
pub fn check_ffmpeg(log: State<'_, SharedErrorLog>) -> Result<bool, String> {
    match crate::video::FfmpegTools::locate() {
        Ok(_) => Ok(true),
        Err(error) => {
            log_rust_error(&log, "check_ffmpeg", &error);
            Ok(false)
        }
    }
}

#[tauri::command]
pub fn trim_current_video(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    trim_start: f64,
    trim_end: f64,
) -> Result<ActionResult, String> {
    wrap(
        &log,
        "trim_current_video",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .trim_current_video(trim_start, trim_end),
    )
}

// ---------------------------------------------------------------------------
// Batch optimization / conversion
// ---------------------------------------------------------------------------

const MEDIA_FILTER_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp", "tiff", "tif", "mp4", "mov", "m4v",
    "avi", "mkv", "3gp",
];

fn contains_heic(paths: &[String]) -> bool {
    paths.iter().any(|path| {
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif")
            })
    })
}

fn requests_unavailable_avif_metadata(paths: &[String], settings: &BatchSettings) -> bool {
    settings.image.keep_metadata
        && settings.image.format == ImageFormat::Avif
        && paths.iter().any(|path| {
            crate::batch::ffmpeg_args::classify_path(Path::new(path)) == Some(BatchMediaType::Image)
        })
}

/// Forwards runner updates to the window. Channel names are prefixed so they
/// cannot collide with other events.
struct WindowEvents {
    app: AppHandle,
    report_dirs: Vec<PathBuf>,
    checkpoint: Option<CheckpointWriter>,
}

#[derive(Clone)]
struct CheckpointWriter {
    store: BatchCheckpointStore,
    checkpoint: Arc<Mutex<BatchCheckpoint>>,
}

impl CheckpointWriter {
    fn save_job(&self, job: BatchJobStatus) {
        let Ok(mut checkpoint) = self.checkpoint.lock() else {
            return;
        };
        checkpoint.update_job(job);
        if let Err(error) = self.store.save(&checkpoint) {
            eprintln!("[QMO][batch] {error}");
        }
    }
}

impl BatchEvents for WindowEvents {
    fn item(&self, item: &BatchItemStatus) {
        let _ = self.app.emit("batch://item", item);
        if matches!(
            item.state,
            crate::batch::BatchItemState::Done
                | crate::batch::BatchItemState::Skipped
                | crate::batch::BatchItemState::Failed
                | crate::batch::BatchItemState::Cancelled
        ) {
            let job_id = self
                .checkpoint
                .as_ref()
                .and_then(|writer| writer.checkpoint.lock().ok().map(|c| c.job.job_id.clone()));
            if let Some(job_id) = job_id {
                let snapshot = self
                    .app
                    .state::<SharedBatchState>()
                    .lock()
                    .ok()
                    .and_then(|runner| runner.snapshot(&job_id));
                if let (Some(writer), Some(snapshot)) = (&self.checkpoint, snapshot) {
                    writer.save_job(snapshot);
                }
            }
        }
    }

    fn progress(&self, summary: &BatchProgressSummary) {
        let _ = self.app.emit("batch://progress", summary);
    }

    fn done(&self, job: &BatchJobStatus) {
        if let Some(writer) = &self.checkpoint {
            writer.save_job(job.clone());
        }
        for dir in &self.report_dirs {
            if let Err(error) = crate::batch::report::write_batch_report(dir, job) {
                eprintln!("[QMO][batch] {error}");
            }
        }
        let _ = self.app.emit("batch://done", job);
    }
}

fn batch_report_dirs(app: &AppHandle) -> Vec<PathBuf> {
    let mut dirs = app
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| vec![dir.join("logs")])
        .unwrap_or_default();
    if cfg!(debug_assertions) {
        if let Ok(cwd) = std::env::current_dir() {
            let debug_dir = cwd.join("logs");
            if !dirs.contains(&debug_dir) {
                dirs.push(debug_dir);
            }
        }
    }
    dirs
}

fn window_events(app: &AppHandle, checkpoint: Option<CheckpointWriter>) -> Arc<WindowEvents> {
    Arc::new(WindowEvents {
        app: app.clone(),
        report_dirs: batch_report_dirs(app),
        checkpoint,
    })
}

/// Restores the active checkpoint during desktop startup. A completed job is
/// registered so the UI can finalize it. A process-interrupted job restarts
/// only its pending files with the output paths saved before shutdown.
pub fn resume_interrupted_batch_job(app: &AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data folder: {error}"))?;
    let store = BatchCheckpointStore::new(&app_data_dir);
    let Some(mut checkpoint) = store.load()? else {
        return Ok(());
    };
    if checkpoint.job.finalized {
        return store.clear();
    }

    let was_running = checkpoint.job.running;
    let plans = was_running.then(|| checkpoint.prepare_resume());
    let job_id = checkpoint.job.job_id.clone();
    let settings = checkpoint.settings.clone();
    let job = Arc::new(Mutex::new(checkpoint.job.clone()));
    let cancel = Arc::new(AtomicBool::new(false));

    if !was_running {
        let batch = app.state::<SharedBatchState>();
        batch
            .lock()
            .map_err(|error| error.to_string())?
            .register(job, cancel, &job_id);
        return Ok(());
    }

    let checkpoint = Arc::new(Mutex::new(checkpoint));
    {
        let guard = checkpoint.lock().map_err(|error| error.to_string())?;
        store.save(&guard)?;
    }
    {
        let batch = app.state::<SharedBatchState>();
        batch.lock().map_err(|error| error.to_string())?.register(
            Arc::clone(&job),
            Arc::clone(&cancel),
            &job_id,
        );
    }
    let writer = CheckpointWriter { store, checkpoint };
    let events = window_events(app, Some(writer.clone()));
    let Some(plans) = plans else {
        return Ok(());
    };
    let encoder = match FfmpegEncoder::locate() {
        Ok(encoder) => Arc::new(encoder),
        Err(error) => {
            let snapshot = {
                let mut guard = job.lock().map_err(|lock_error| lock_error.to_string())?;
                for item in &mut guard.items {
                    if item.state == crate::batch::BatchItemState::Pending {
                        item.state = crate::batch::BatchItemState::Failed;
                        item.error = Some(format!("Could not resume after restart: {error}"));
                    }
                }
                guard.running = false;
                guard.failed = guard
                    .items
                    .iter()
                    .filter(|item| item.state == crate::batch::BatchItemState::Failed)
                    .count();
                guard.finished_at = Some(chrono::Local::now().to_rfc3339());
                guard.clone()
            };
            writer.save_job(snapshot);
            return Ok(());
        }
    };

    let thread_job = Arc::clone(&job);
    std::thread::spawn(move || {
        crate::batch::runner::run_job(thread_job, plans, settings, encoder, events, cancel);
    });
    Ok(())
}

#[tauri::command]
pub fn list_queue_items(state: State<'_, SharedState>) -> Result<Vec<MediaItem>, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.list_items())
}

/// Metadata for files picked outside the open album.
#[tauri::command]
pub fn describe_media_paths(paths: Vec<String>) -> Result<Vec<MediaItem>, String> {
    Ok(paths
        .iter()
        .filter(|path| Path::new(path).is_file())
        .map(|path| {
            let mut item = crate::media::build_media_item_from_paths(std::slice::from_ref(path));
            crate::media::enrich_item_metadata(&mut item);
            item
        })
        .collect())
}

#[tauri::command]
pub async fn pick_media_files(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let files = app
        .dialog()
        .file()
        .add_filter("Photos and videos", MEDIA_FILTER_EXTENSIONS)
        .blocking_pick_files();

    Ok(files
        .unwrap_or_default()
        .into_iter()
        .map(|file| file.to_string())
        .collect())
}

#[tauri::command]
pub fn get_ffmpeg_capabilities(
    log: State<'_, SharedErrorLog>,
) -> Result<FfmpegCapabilities, String> {
    match crate::video::FfmpegTools::locate() {
        Ok(tools) => Ok(tools.capabilities()),
        Err(error) => {
            log_rust_error(&log, "get_ffmpeg_capabilities", &error);
            Ok(FfmpegCapabilities::default())
        }
    }
}

#[tauri::command]
pub fn get_batch_presets(state: State<'_, SharedState>) -> Result<Vec<BatchPreset>, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.batch_presets())
}

#[tauri::command]
pub fn save_batch_preset(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    preset: BatchPreset,
) -> Result<Vec<BatchPreset>, String> {
    wrap(
        &log,
        "save_batch_preset",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .save_batch_preset(preset),
    )
}

#[tauri::command]
pub fn delete_batch_preset(
    state: State<'_, SharedState>,
    log: State<'_, SharedErrorLog>,
    id: String,
) -> Result<Vec<BatchPreset>, String> {
    wrap(
        &log,
        "delete_batch_preset",
        state
            .lock()
            .map_err(|e| e.to_string())?
            .delete_batch_preset(&id),
    )
}

#[tauri::command]
pub fn get_last_batch_settings(
    state: State<'_, SharedState>,
) -> Result<Option<BatchSettings>, String> {
    Ok(state
        .lock()
        .map_err(|e| e.to_string())?
        .app_settings
        .last_batch_settings
        .clone())
}

#[tauri::command]
pub async fn estimate_batch_size(
    app: AppHandle,
    paths: Vec<String>,
    settings: BatchSettings,
) -> Result<BatchEstimate, String> {
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        (|| {
            if paths.is_empty() {
                return Err("Select at least one photo or video.".into());
            }
            let encoder = FfmpegEncoder::locate()?;
            let capabilities = encoder.capabilities();
            let cache = worker_app
                .path()
                .app_cache_dir()
                .map_err(|error| format!("Could not resolve cache folder: {error}"))?
                .join("batch-estimates");
            crate::batch::estimate::estimate_batch(
                &paths,
                &settings,
                &capabilities,
                &encoder,
                &cache,
                &AtomicBool::new(false),
            )
        })()
    })
    .await
    .map_err(|error| format!("Estimate worker failed: {error}"))?;
    if let Err(error) = &result {
        let log = app.state::<SharedErrorLog>();
        log_rust_error(&log, "estimate_batch_size", error);
    }
    result
}

/// Starts a batch job. Long encodes run on their own threads: the app-state
/// mutex is never held while ffmpeg works.
#[tauri::command]
pub fn start_batch_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    batch: State<'_, SharedBatchState>,
    log: State<'_, SharedErrorLog>,
    paths: Vec<String>,
    settings: BatchSettings,
) -> Result<BatchJobStatus, String> {
    wrap(
        &log,
        "start_batch_job",
        (|| {
            if paths.is_empty() {
                return Err("Select at least one photo or video.".into());
            }

            // Destructive runs must carry the confirmation the dialog sets. This is
            // checked before a single file is touched.
            if let OutputMode::ReplaceOriginal { confirmed, .. } = &settings.output {
                if !confirmed {
                    return Err(
                    "Replacing originals was not confirmed — open the confirmation dialog first."
                        .into(),
                );
                }
            }

            let encoder = Arc::new(FfmpegEncoder::locate()?);
            let capabilities = encoder.capabilities();
            if contains_heic(&paths) && !capabilities.heic_decode {
                return Err(
                "This FFmpeg build cannot read HEIC/HEIF. Install a full FFmpeg build before converting those files."
                    .into(),
            );
            }
            if requests_unavailable_avif_metadata(&paths, &settings) {
                return Err(
                    "AVIF metadata preservation is unavailable. Disable metadata preservation or choose another image format."
                        .into(),
                );
            }
            let contains_video = paths.iter().any(|path| {
                crate::batch::ffmpeg_args::classify_path(Path::new(path))
                    == Some(BatchMediaType::Video)
            });
            if contains_video {
                resolve_video_encoding(&settings.video, &capabilities.video_backends)?;
            }

            {
                let runner = batch.lock().map_err(|e| e.to_string())?;
                if let Some(active) = runner.active_job_id() {
                    if let Some(job) = runner.snapshot(&active) {
                        if job.running {
                            return Err("A batch job is already running.".into());
                        }
                        if !job.finalized {
                            return Err(
                                "Finish the previous batch result before starting another job."
                                    .into(),
                            );
                        }
                    }
                }
            }

            let plans =
                plan_items_with_capabilities(&paths, &settings, &capabilities.video_backends);
            let statuses: Vec<BatchItemStatus> =
                plans.iter().map(|planned| planned.status.clone()).collect();
            let output_dir = plans
                .iter()
                .find_map(|planned| planned.plan.as_ref())
                .and_then(|plan| plan.final_output.parent())
                .map(|dir| dir.to_string_lossy().to_string());
            let replaces_originals = matches!(settings.output, OutputMode::ReplaceOriginal { .. });

            let job_id = {
                let mut runner = batch.lock().map_err(|e| e.to_string())?;
                runner.next_job_id()
            };
            let job = Arc::new(Mutex::new(new_job_status(
                job_id.clone(),
                statuses,
                output_dir,
                replaces_originals,
            )));
            let cancel = Arc::new(AtomicBool::new(false));

            if let Ok(mut guard) = state.lock() {
                let _ = guard.remember_batch_settings(&settings);
            }

            let snapshot = job.lock().map_err(|e| e.to_string())?.clone();
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("Could not resolve app data folder: {error}"))?;
            let store = BatchCheckpointStore::new(&app_data_dir);
            let checkpoint = Arc::new(Mutex::new(BatchCheckpoint::new(
                snapshot.clone(),
                settings.clone(),
                &plans,
            )));
            {
                let guard = checkpoint.lock().map_err(|error| error.to_string())?;
                store.save(&guard)?;
            }
            {
                let mut runner = batch.lock().map_err(|e| e.to_string())?;
                runner.register(Arc::clone(&job), Arc::clone(&cancel), &job_id);
            }
            let events = window_events(&app, Some(CheckpointWriter { store, checkpoint }));
            let thread_job = Arc::clone(&job);
            std::thread::spawn(move || {
                crate::batch::runner::run_job(thread_job, plans, settings, encoder, events, cancel);
            });

            Ok(snapshot)
        })(),
    )
}

#[tauri::command]
pub fn cancel_batch_job(batch: State<'_, SharedBatchState>, job_id: String) -> Result<(), String> {
    batch.lock().map_err(|e| e.to_string())?.cancel(&job_id)
}

#[tauri::command]
pub fn get_batch_job(
    batch: State<'_, SharedBatchState>,
    job_id: String,
) -> Result<BatchJobStatus, String> {
    batch
        .lock()
        .map_err(|e| e.to_string())?
        .snapshot(&job_id)
        .ok_or_else(|| format!("Unknown batch job: {job_id}"))
}

/// Applies a finished job to the open session: registers the undo entry for
/// replaced originals and rebuilds the queue. Safe to call twice.
#[tauri::command]
pub fn finalize_batch_job(
    app: AppHandle,
    state: State<'_, SharedState>,
    batch: State<'_, SharedBatchState>,
    log: State<'_, SharedErrorLog>,
    job_id: String,
) -> Result<FrontendState, String> {
    wrap(
        &log,
        "finalize_batch_job",
        (|| {
            let (job_handle, replacements) = {
                let runner = batch.lock().map_err(|e| e.to_string())?;
                let handle = runner
                    .job(&job_id)
                    .ok_or_else(|| format!("Unknown batch job: {job_id}"))?;
                let guard = handle.lock().map_err(|e| e.to_string())?;
                if guard.running {
                    return Err("The batch job is still running.".into());
                }
                let replacements = if guard.finalized {
                    Vec::new()
                } else {
                    guard
                        .replacements
                        .iter()
                        .map(|r| {
                            (
                                r.backup_path.clone(),
                                r.original_path.clone(),
                                r.converted_path.clone(),
                            )
                        })
                        .collect()
                };
                drop(guard);
                (handle, replacements)
            };

            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.apply_batch_replacements(&replacements)?;
            let frontend = guard.to_frontend_state();
            drop(guard);
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|error| format!("Could not resolve app data folder: {error}"))?;
            let store = BatchCheckpointStore::new(&app_data_dir);
            if let Some(mut checkpoint) = store.load()? {
                checkpoint.job.finalized = true;
                store.save(&checkpoint)?;
            }

            job_handle
                .lock()
                .map_err(|error| error.to_string())?
                .finalized = true;
            batch
                .lock()
                .map_err(|error| error.to_string())?
                .clear_active(&job_id);
            store.clear()?;
            Ok(frontend)
        })(),
    )
}

/// Duration of one file, for the batch selection list.
#[tauri::command]
pub fn probe_video_duration(path: String) -> Result<Option<f64>, String> {
    let Ok(encoder) = FfmpegEncoder::locate() else {
        return Ok(None);
    };
    Ok(encoder.probe_duration(Path::new(&path)))
}

/// Lists the media in a folder for a batch run, without opening it as an
/// album (no session file is written). `exclude_dirs` keeps a previous run's
/// output from being fed back in.
#[tauri::command]
pub fn scan_folder_media(
    log: State<'_, SharedErrorLog>,
    path: String,
    recursive: bool,
    exclude_dirs: Vec<String>,
    exclude_dir_names: Vec<String>,
) -> Result<Vec<MediaItem>, String> {
    wrap(
        &log,
        "scan_folder_media",
        (|| {
            let root = PathBuf::from(&path);
            if !root.is_dir() {
                return Err(format!("{path} is not a folder."));
            }

            let excluded: Vec<PathBuf> = exclude_dirs
                .iter()
                .filter(|dir| !dir.trim().is_empty())
                .map(PathBuf::from)
                .collect();
            // Output subfolders live *under* the scanned root, one per source
            // folder, so they can only be excluded by name: a recursive scan
            // would otherwise queue everything the last run just produced.
            let excluded_names: Vec<String> = exclude_dir_names
                .iter()
                .map(|name| name.trim().to_lowercase())
                .filter(|name| !name.is_empty())
                .collect();

            let items = crate::media::scan_folder(&root, recursive)?;
            Ok(items
                .into_iter()
                .filter(|item| {
                    !item.paths.iter().any(|file| {
                        let file = Path::new(file);
                        excluded
                            .iter()
                            .any(|dir| crate::path_util::is_path_inside_root(dir, file))
                            || is_under_named_dir(&root, file, &excluded_names)
                    })
                })
                .collect())
        })(),
    )
}

/// True when any folder between `root` and `file` carries an excluded name.
/// Only the part below the root is inspected: a user whose album happens to
/// sit in `D:/_optimized` still gets their files.
fn is_under_named_dir(root: &Path, file: &Path, names: &[String]) -> bool {
    if names.is_empty() {
        return false;
    }
    let Ok(relative) = file.strip_prefix(root) else {
        return false;
    };
    let mut components: Vec<_> = relative.components().collect();
    components.pop(); // The file name itself is not a folder.
    components
        .iter()
        .any(|component| names.contains(&component.as_os_str().to_string_lossy().to_lowercase()))
}

/// The job currently registered, if any. Lets the panel re-attach to a run
/// that survived a window reload.
#[tauri::command]
pub fn get_active_batch_job(
    batch: State<'_, SharedBatchState>,
) -> Result<Option<BatchJobStatus>, String> {
    let runner = batch.lock().map_err(|e| e.to_string())?;
    Ok(runner
        .active_job_id()
        .and_then(|job_id| runner.snapshot(&job_id)))
}

/// What the UI needs to talk about updates without hardcoding a repository.
#[derive(serde::Serialize)]
pub struct UpdateContext {
    pub current_version: String,
    /// True when this build was signed and given an update endpoint.
    pub updater_configured: bool,
    /// Releases page of whichever repository published this build.
    pub releases_url: Option<String>,
    /// Repository this build came from, for the "view on GitHub" link.
    pub repository_url: Option<String>,
}

/// Derives the repository from the updater endpoint baked into the build, so a
/// fork points at its own releases with nothing to edit by hand.
fn releases_url_from_endpoint(endpoint: &str) -> Option<String> {
    let marker = "/releases/";
    let index = endpoint.find(marker)?;
    Some(format!("{}/releases", &endpoint[..index]))
}

#[tauri::command]
pub fn get_update_context(app: AppHandle) -> Result<UpdateContext, String> {
    let updater = app
        .config()
        .plugins
        .0
        .get("updater")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    let endpoint = updater
        .get("endpoints")
        .and_then(|endpoints| endpoints.as_array())
        .and_then(|endpoints| endpoints.first())
        .and_then(|endpoint| endpoint.as_str())
        .map(str::to_string);

    let releases_url = endpoint.as_deref().and_then(releases_url_from_endpoint);
    let repository_url = releases_url
        .as_deref()
        .and_then(|url| url.strip_suffix("/releases").map(str::to_string))
        // Builds without an updater still know where they came from: build.rs
        // records it from CI or from the git remote.
        .or_else(|| option_env!("QMO_REPOSITORY_URL").map(str::to_string));

    Ok(UpdateContext {
        current_version: app.package_info().version.to_string(),
        updater_configured: endpoint.is_some(),
        releases_url,
        repository_url,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        contains_heic, is_under_named_dir, releases_url_from_endpoint,
        requests_unavailable_avif_metadata,
    };
    use crate::batch::{BatchSettings, ImageFormat};
    use std::path::Path;

    #[test]
    fn detects_heic_inputs_case_insensitively() {
        assert!(contains_heic(&["C:\\Photos\\IMG_1.HEIC".into()]));
        assert!(contains_heic(&["photo.heif".into()]));
        assert!(!contains_heic(&["photo.jpg".into(), "clip.mov".into()]));
    }

    #[test]
    fn rejects_avif_metadata_only_when_images_are_selected() {
        let mut settings = BatchSettings::default();
        settings.image.format = ImageFormat::Avif;
        settings.image.keep_metadata = true;

        assert!(requests_unavailable_avif_metadata(
            &["photo.jpg".into()],
            &settings
        ));
        assert!(!requests_unavailable_avif_metadata(
            &["clip.mov".into()],
            &settings
        ));
        settings.image.keep_metadata = false;
        assert!(!requests_unavailable_avif_metadata(
            &["photo.jpg".into()],
            &settings
        ));
    }

    #[test]
    fn derives_the_releases_page_from_the_update_endpoint() {
        assert_eq!(
            releases_url_from_endpoint(
                "https://github.com/someone/quick-media-organizer/releases/latest/download/latest.json"
            )
            .as_deref(),
            Some("https://github.com/someone/quick-media-organizer/releases")
        );
        assert_eq!(
            releases_url_from_endpoint("https://example.com/feed.json"),
            None
        );
    }

    #[test]
    fn an_output_subfolder_is_excluded_at_any_depth() {
        let root = Path::new("D:/camera");
        let names = vec!["_optimized".to_string()];

        assert!(is_under_named_dir(
            root,
            Path::new("D:/camera/_optimized/a.jpg"),
            &names
        ));
        // Case does not match on Windows, and depth must not matter.
        assert!(is_under_named_dir(
            root,
            Path::new("D:/camera/trip/_Optimized/a.jpg"),
            &names
        ));
        assert!(!is_under_named_dir(
            root,
            Path::new("D:/camera/trip/a.jpg"),
            &names
        ));
        // A file *named* like the folder is still a file to convert.
        assert!(!is_under_named_dir(
            root,
            Path::new("D:/camera/_optimized.jpg"),
            &names
        ));
        // The root itself is never inspected: an album can live anywhere.
        assert!(!is_under_named_dir(
            Path::new("D:/_optimized"),
            Path::new("D:/_optimized/a.jpg"),
            &names
        ));
        assert!(!is_under_named_dir(root, Path::new("D:/camera/a.jpg"), &[]));
    }

    #[test]
    fn the_build_records_where_it_came_from() {
        // build.rs resolves this from CI or the git remote; a checkout without
        // either simply has no link to show.
        if let Some(url) = option_env!("QMO_REPOSITORY_URL") {
            assert!(
                url.starts_with("https://"),
                "unexpected repository url: {url}"
            );
            assert!(
                !url.ends_with(".git"),
                "the .git suffix must be stripped: {url}"
            );
        }
    }
}
