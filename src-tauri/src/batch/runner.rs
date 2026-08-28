//! Batch job runner.
//!
//! Deliberately independent from `AppState`: encoding a folder takes minutes,
//! and holding the app-state mutex for that long would freeze every other
//! command. Workers only touch the job's own mutex, for microseconds at a time.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::ffmpeg_args::{
    classify_path, image_flags, output_extension, output_file_name, resolve_output_path,
    temp_output_path,
};
#[cfg(test)]
use super::video_backend::software_capability;
use super::video_backend::{
    backend_label, next_attempt_index, resolve_video_encoding, VideoEncodeAttempt,
    VideoEncodingPlan,
};
use super::{
    AudioMode, BatchItemState, BatchItemStatus, BatchJobStatus, BatchMediaType,
    BatchProgressSummary, BatchReplacement, BatchSettings, ConflictPolicy, OutputMode,
    VideoBackend, VideoBackendCapability,
};
use crate::fs_util::{apply_timestamps, copy_file_preserve, read_timestamps};
use crate::path_util::{is_path_inside_root, APP_FOLDER_NAME};
use crate::video::{EncodeRequest, FfmpegTools, CANCELLED};

const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(250);

pub type SharedBatchState = Mutex<BatchRunner>;

/// Registry of jobs. One job runs at a time; finished jobs stay around so the
/// UI can re-attach after a window reload.
#[derive(Default)]
pub struct BatchRunner {
    jobs: HashMap<String, Arc<Mutex<BatchJobStatus>>>,
    cancels: HashMap<String, Arc<AtomicBool>>,
    /// Registration order, so the oldest job is the one that gets forgotten.
    order: Vec<String>,
    active: Option<String>,
    counter: u64,
}

/// Finished jobs are kept so the panel can re-attach after a window reload,
/// but each one holds a status entry per file: only the most recent few stay.
const MAX_REMEMBERED_JOBS: usize = 4;

impl BatchRunner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_job_id(&mut self) -> String {
        self.counter += 1;
        format!(
            "job-{}-{}",
            chrono::Local::now().format("%Y%m%d%H%M%S"),
            self.counter
        )
    }

    pub fn active_job_id(&self) -> Option<String> {
        self.active.clone()
    }

    pub fn register(
        &mut self,
        job: Arc<Mutex<BatchJobStatus>>,
        cancel: Arc<AtomicBool>,
        job_id: &str,
    ) {
        self.jobs.insert(job_id.to_string(), job);
        self.cancels.insert(job_id.to_string(), cancel);
        self.active = Some(job_id.to_string());

        self.order.retain(|id| id != job_id);
        self.order.push(job_id.to_string());
        // The job just registered is last, so it is never the one dropped.
        while self.order.len() > MAX_REMEMBERED_JOBS {
            let oldest = self.order.remove(0);
            self.jobs.remove(&oldest);
            self.cancels.remove(&oldest);
        }
    }

    pub fn job(&self, job_id: &str) -> Option<Arc<Mutex<BatchJobStatus>>> {
        self.jobs.get(job_id).cloned()
    }

    pub fn snapshot(&self, job_id: &str) -> Option<BatchJobStatus> {
        self.jobs
            .get(job_id)
            .and_then(|job| job.lock().ok().map(|guard| guard.clone()))
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), String> {
        let flag = self
            .cancels
            .get(job_id)
            .ok_or_else(|| format!("Unknown batch job: {job_id}"))?;
        flag.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn clear_active(&mut self, job_id: &str) {
        if self.active.as_deref() == Some(job_id) {
            self.active = None;
        }
    }
}

/// Encoding backend. Abstracted so the runner can be tested without ffmpeg.
pub trait MediaEncoder: Send + Sync {
    fn probe_duration(&self, path: &Path) -> Option<f64>;

    fn encode(&self, request: EncodeRequest<'_>) -> Result<(), String>;
}

pub struct FfmpegEncoder {
    tools: FfmpegTools,
}

impl FfmpegEncoder {
    pub fn locate() -> Result<Self, String> {
        Ok(Self {
            tools: FfmpegTools::locate()?,
        })
    }

    pub fn capabilities(&self) -> crate::batch::FfmpegCapabilities {
        self.tools.capabilities()
    }
}

impl MediaEncoder for FfmpegEncoder {
    fn probe_duration(&self, path: &Path) -> Option<f64> {
        self.tools.probe_duration(path).ok()
    }

    fn encode(&self, request: EncodeRequest<'_>) -> Result<(), String> {
        self.tools.encode(request)
    }
}

/// Where job updates go. `TauriEvents` emits to the window; tests collect them.
pub trait BatchEvents: Send + Sync {
    fn item(&self, item: &BatchItemStatus);
    fn progress(&self, summary: &BatchProgressSummary);
    fn done(&self, job: &BatchJobStatus);
}

/// One unit of work, fully resolved before any worker starts, so two workers
/// can never race for the same output name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodePlan {
    pub input: PathBuf,
    pub temp_output: PathBuf,
    pub final_output: PathBuf,
    /// Ordered attempts. Video plans may move from hardware to software and
    /// from copied audio to AAC according to the classified FFmpeg error.
    pub attempts: Vec<VideoEncodeAttempt>,
    pub resolved_backend: Option<VideoBackend>,
    pub media_type: BatchMediaType,
    pub replace_original: bool,
    pub backup: bool,
    /// A different file already occupying `final_output`. Overwrite is allowed
    /// only after this file has its own backup and rollback path.
    pub overwritten_target: Option<PathBuf>,
}

pub struct PlannedItem {
    pub status: BatchItemStatus,
    pub plan: Option<EncodePlan>,
}

/// Builds the work list. Unsupported or unreadable files come back already
/// marked as failed/skipped instead of blowing up the whole job.
#[cfg(test)]
pub fn plan_items(paths: &[String], settings: &BatchSettings) -> Vec<PlannedItem> {
    plan_items_with_capabilities(paths, settings, &[software_capability()])
}

pub fn plan_items_with_capabilities(
    paths: &[String],
    settings: &BatchSettings,
    video_backends: &[VideoBackendCapability],
) -> Vec<PlannedItem> {
    let mut reserved: Vec<PathBuf> = Vec::new();
    let mut planned: Vec<PlannedItem> = Vec::new();

    for (index, raw) in paths.iter().enumerate() {
        let source = PathBuf::from(raw);
        let file_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(raw)
            .to_string();
        let media_type = classify_path(&source);
        let size_before = fs::metadata(&source).map(|m| m.len()).unwrap_or(0);

        let mut status = BatchItemStatus {
            id: raw.clone(),
            source_path: raw.clone(),
            file_name,
            media_type: media_type.unwrap_or(BatchMediaType::Image),
            state: BatchItemState::Pending,
            progress: 0.0,
            size_before,
            size_after: None,
            output_path: None,
            error: None,
            encoder_backend: None,
            fallback_reason: None,
        };

        let Some(media_type) = media_type else {
            status.state = BatchItemState::Failed;
            status.error = Some("Unsupported file type.".into());
            planned.push(PlannedItem { status, plan: None });
            continue;
        };

        if !source.is_file() {
            status.state = BatchItemState::Failed;
            status.error = Some("File not found.".into());
            planned.push(PlannedItem { status, plan: None });
            continue;
        }

        match build_plan(
            &source,
            media_type,
            settings,
            index,
            &reserved,
            video_backends,
        ) {
            Ok(PlanOutcome::Work(plan)) => {
                reserved.push(plan.final_output.clone());
                status.output_path = Some(plan.final_output.to_string_lossy().to_string());
                planned.push(PlannedItem {
                    status,
                    plan: Some(*plan),
                });
            }
            Ok(PlanOutcome::Skip(reason)) => {
                status.state = BatchItemState::Skipped;
                status.error = Some(reason);
                planned.push(PlannedItem { status, plan: None });
            }
            Err(error) => {
                status.state = BatchItemState::Failed;
                status.error = Some(error);
                planned.push(PlannedItem { status, plan: None });
            }
        }
    }

    planned
}

/// Outcome of planning one file: real work, or a reason to leave it alone.
enum PlanOutcome {
    Work(Box<EncodePlan>),
    Skip(String),
}

fn build_plan(
    source: &Path,
    media_type: BatchMediaType,
    settings: &BatchSettings,
    index: usize,
    reserved: &[PathBuf],
    video_backends: &[VideoBackendCapability],
) -> Result<PlanOutcome, String> {
    let ext = output_extension(media_type, settings, source)?;
    let (attempts, resolved_backend) = match media_type {
        BatchMediaType::Video => {
            let video = resolve_video_encoding(&settings.video, video_backends)?;
            (video.attempts, Some(video.resolved_backend))
        }
        BatchMediaType::Image => (
            vec![VideoEncodeAttempt {
                backend: VideoBackend::Software,
                audio: AudioMode::Drop,
                input_flags: Vec::new(),
                output_flags: image_flags(&settings.image, &ext)?,
            }],
            None,
        ),
    };

    let source_dir = source
        .parent()
        .ok_or_else(|| "File has no parent folder.".to_string())?
        .to_path_buf();

    let (final_output, replace_original, backup, overwritten_target) = match &settings.output {
        OutputMode::ReplaceOriginal { backup, confirmed } => {
            if !confirmed {
                return Err(
                    "Replacing originals was not confirmed. Re-open the confirmation dialog."
                        .into(),
                );
            }
            let requested = source.with_extension(&ext);
            if requested == source {
                (requested, true, *backup, None)
            } else {
                let reserved_collision = reserved.iter().any(|taken| taken == &requested);
                if reserved_collision && settings.on_conflict == ConflictPolicy::Overwrite {
                    return Err(format!(
                        "Two selected files resolve to the same output: {}.",
                        requested.display()
                    ));
                }

                let occupied = requested.exists() || reserved_collision;
                match (occupied, settings.on_conflict) {
                    (false, _) => (requested, true, *backup, None),
                    (true, ConflictPolicy::Skip) => {
                        return Ok(PlanOutcome::Skip("A converted file already exists.".into()));
                    }
                    (true, ConflictPolicy::Rename) => {
                        let exists = |candidate: &Path| {
                            candidate.exists() || reserved.iter().any(|taken| taken == candidate)
                        };
                        let resolved = resolve_output_path(
                            source,
                            &source_dir,
                            &ext,
                            None,
                            ConflictPolicy::Rename,
                            &exists,
                        )?
                        .ok_or_else(|| "Could not resolve a replacement name.".to_string())?;
                        (resolved, true, *backup, None)
                    }
                    (true, ConflictPolicy::Overwrite) => {
                        (requested.clone(), true, *backup, Some(requested))
                    }
                }
            }
        }
        mode => {
            let dest_dir = match mode {
                OutputMode::Subfolder { name } => source_dir.join(sanitize_folder_name(name)),
                OutputMode::CustomFolder { path } => PathBuf::from(path),
                OutputMode::ReplaceOriginal { .. } => unreachable!(),
            };

            // Never re-compress what a previous run already produced.
            if is_path_inside_root(&dest_dir, source) {
                return Ok(PlanOutcome::Skip(
                    "Already in the output folder — skipped so it is not re-compressed.".into(),
                ));
            }

            fs::create_dir_all(&dest_dir)
                .map_err(|e| format!("Cannot create {}: {e}", dest_dir.display()))?;

            let requested = dest_dir.join(output_file_name(
                source,
                &ext,
                settings.name_suffix.as_deref(),
            ));
            if settings.on_conflict == ConflictPolicy::Overwrite
                && reserved.iter().any(|taken| taken == &requested)
            {
                return Err(format!(
                    "Two selected files resolve to the same output: {}.",
                    requested.display()
                ));
            }

            let exists = |candidate: &Path| {
                candidate.exists() || reserved.iter().any(|taken| taken == candidate)
            };
            let Some(resolved) = resolve_output_path(
                source,
                &dest_dir,
                &ext,
                settings.name_suffix.as_deref(),
                settings.on_conflict,
                &exists,
            )?
            else {
                return Ok(PlanOutcome::Skip("A converted file already exists.".into()));
            };
            (resolved, false, false, None)
        }
    };

    Ok(PlanOutcome::Work(Box::new(EncodePlan {
        input: source.to_path_buf(),
        temp_output: temp_output_path(&final_output, index),
        final_output,
        attempts,
        resolved_backend,
        media_type,
        replace_original,
        backup,
        overwritten_target,
    })))
}

fn sanitize_folder_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    if cleaned.is_empty() {
        "_optimized".to_string()
    } else {
        cleaned
    }
}

/// Runs every planned item, `settings.concurrency` at a time.
pub fn run_job<E, V>(
    job: Arc<Mutex<BatchJobStatus>>,
    plans: Vec<PlannedItem>,
    settings: BatchSettings,
    encoder: Arc<E>,
    events: Arc<V>,
    cancel: Arc<AtomicBool>,
) where
    E: MediaEncoder + 'static,
    V: BatchEvents + 'static,
{
    let work: Vec<(usize, EncodePlan)> = plans
        .iter()
        .enumerate()
        .filter_map(|(index, planned)| planned.plan.clone().map(|plan| (index, plan)))
        .collect();

    // Rebuild aggregates from statuses so a resumed job keeps its completed
    // work while a fresh job counts planning failures exactly once.
    recalculate_terminal_state(&job);

    // Items that never got a plan already carry their final state.
    for planned in &plans {
        if planned.plan.is_none() {
            let status = planned.status.clone();
            events.item(&status);
        }
    }
    emit_progress(&job, events.as_ref());

    let next = Arc::new(AtomicUsize::new(0));
    let workers = settings.concurrency.clamp(1, 8).min(work.len().max(1));
    let work = Arc::new(work);
    // Many consumer GPU drivers become unstable when several encoders are
    // opened concurrently. CPU and still-image work remain fully concurrent.
    let hardware_gate = Arc::new(Mutex::new(()));

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let next = Arc::clone(&next);
            let work = Arc::clone(&work);
            let job = Arc::clone(&job);
            let encoder = Arc::clone(&encoder);
            let events = Arc::clone(&events);
            let cancel = Arc::clone(&cancel);
            let settings = settings.clone();
            let hardware_gate = Arc::clone(&hardware_gate);

            scope.spawn(move || loop {
                let slot = next.fetch_add(1, Ordering::Relaxed);
                if slot >= work.len() || cancel.load(Ordering::Relaxed) {
                    break;
                }
                let (index, plan) = &work[slot];
                let context = ProcessContext {
                    settings: &settings,
                    job: &job,
                    encoder: encoder.as_ref(),
                    events: events.as_ref(),
                    cancel: &cancel,
                    hardware_gate: &hardware_gate,
                };
                process_one(*index, plan, &context);
            });
        }
    });

    finish_job(&job, &cancel, events.as_ref());
}

struct ProcessContext<'a, E, V> {
    settings: &'a BatchSettings,
    job: &'a Arc<Mutex<BatchJobStatus>>,
    encoder: &'a E,
    events: &'a V,
    cancel: &'a AtomicBool,
    hardware_gate: &'a Mutex<()>,
}

fn process_one<E: MediaEncoder, V: BatchEvents>(
    index: usize,
    plan: &EncodePlan,
    context: &ProcessContext<'_, E, V>,
) {
    let settings = context.settings;
    let job = context.job;
    let encoder = context.encoder;
    let events = context.events;
    let cancel = context.cancel;
    let hardware_gate = context.hardware_gate;
    if cancel.load(Ordering::Relaxed) {
        return;
    }

    if let Some(status) = update_item(job, index, |item| {
        item.state = BatchItemState::Running;
        item.progress = 0.0;
        item.encoder_backend = plan.resolved_backend;
    }) {
        events.item(&status);
    }

    let duration = match plan.media_type {
        BatchMediaType::Video => encoder.probe_duration(&plan.input),
        BatchMediaType::Image => None,
    };

    let last_emit = Mutex::new(Instant::now() - PROGRESS_EVENT_INTERVAL);
    let on_progress = |progress: f32| {
        let mut guard = match last_emit.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if guard.elapsed() < PROGRESS_EVENT_INTERVAL && progress < 1.0 {
            return;
        }
        *guard = Instant::now();
        drop(guard);
        if let Some(status) = update_item(job, index, |item| {
            item.progress = progress;
        }) {
            events.item(&status);
        }
    };

    let video_plan = plan
        .resolved_backend
        .map(|resolved_backend| VideoEncodingPlan {
            resolved_backend,
            attempts: plan.attempts.clone(),
        });
    let mut attempted = HashSet::new();
    let mut current = 0usize;
    let mut fallback_reason: Option<String> = None;
    let attempt = loop {
        let Some(encode_attempt) = plan.attempts.get(current) else {
            break Err("No encoding attempt was configured.".into());
        };
        if let Some(status) = update_item(job, index, |item| {
            if plan.media_type == BatchMediaType::Video {
                item.encoder_backend = Some(encode_attempt.backend);
            }
            item.fallback_reason = fallback_reason.clone();
        }) {
            events.item(&status);
        }

        let result = if encode_attempt.backend.is_hardware() {
            let _guard = hardware_gate
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            encoder.encode(EncodeRequest {
                input: &plan.input,
                output: &plan.temp_output,
                input_flags: &encode_attempt.input_flags,
                output_flags: &encode_attempt.output_flags,
                total_duration: duration,
                cancel,
                on_progress: &on_progress,
            })
        } else {
            encoder.encode(EncodeRequest {
                input: &plan.input,
                output: &plan.temp_output,
                input_flags: &encode_attempt.input_flags,
                output_flags: &encode_attempt.output_flags,
                total_duration: duration,
                cancel,
                on_progress: &on_progress,
            })
        };

        match result {
            Ok(()) => break Ok(()),
            Err(error) => {
                attempted.insert(current);
                let next = video_plan
                    .as_ref()
                    .and_then(|video| next_attempt_index(video, current, &error, &attempted));
                let Some(next) = next else {
                    break Err(error);
                };
                let next_attempt = &plan.attempts[next];
                fallback_reason = Some(format!(
                    "{} failed; retried with {}: {}",
                    backend_label(encode_attempt.backend),
                    backend_label(next_attempt.backend),
                    concise_error(&error)
                ));
                let _ = fs::remove_file(&plan.temp_output);
                current = next;
            }
        }
    };

    let outcome = attempt.and_then(|()| finalize_output(plan, settings, encoder, job));

    let status = match outcome {
        Ok(ItemOutcome::Done {
            output_path,
            size_after,
        }) => update_item(job, index, |item| {
            item.state = BatchItemState::Done;
            item.progress = 1.0;
            item.size_after = Some(size_after);
            item.output_path = Some(output_path.clone());
            item.error = None;
            item.encoder_backend = plan
                .attempts
                .get(current)
                .filter(|_| plan.media_type == BatchMediaType::Video)
                .map(|attempt| attempt.backend);
            item.fallback_reason = fallback_reason.clone();
        }),
        Ok(ItemOutcome::Skipped { reason }) => update_item(job, index, |item| {
            item.state = BatchItemState::Skipped;
            item.progress = 1.0;
            item.error = Some(reason.clone());
        }),
        Err(error) if error == CANCELLED => {
            let _ = fs::remove_file(&plan.temp_output);
            update_item(job, index, |item| {
                item.state = BatchItemState::Cancelled;
            })
        }
        Err(error) => {
            let _ = fs::remove_file(&plan.temp_output);
            update_item(job, index, |item| {
                item.state = BatchItemState::Failed;
                item.error = Some(error.clone());
                item.fallback_reason = fallback_reason.clone();
            })
        }
    };

    if let Some(status) = status {
        apply_terminal_state(job, &status);
        events.item(&status);
        emit_progress(job, events);
    }
}

fn concise_error(error: &str) -> String {
    error
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(error)
        .trim()
        .chars()
        .take(240)
        .collect()
}

enum ItemOutcome {
    Done {
        output_path: String,
        size_after: u64,
    },
    Skipped {
        reason: String,
    },
}

/// Verifies the temp file, applies the savings rules, then puts it in place.
/// The original is only removed once the replacement is proven good.
fn finalize_output<E: MediaEncoder>(
    plan: &EncodePlan,
    settings: &BatchSettings,
    encoder: &E,
    job: &Arc<Mutex<BatchJobStatus>>,
) -> Result<ItemOutcome, String> {
    let mut size_after = fs::metadata(&plan.temp_output)
        .map_err(|e| format!("Converted file missing: {e}"))?
        .len();
    if size_after == 0 {
        let _ = fs::remove_file(&plan.temp_output);
        return Err("Converted file is empty.".into());
    }

    // ffmpeg writes no EXIF for stills, so the block is carried over by hand.
    // Done before the size rules so the saving reported to the user counts it,
    // A rewrite error is fatal when metadata preservation was requested. A
    // source without EXIF is still valid and returns Ok(false).
    if plan.media_type == BatchMediaType::Image && settings.image.keep_metadata {
        crate::metadata::copy_exif(&plan.input, &plan.temp_output)
            .map_err(|error| format!("Could not preserve image metadata: {error}"))?;
        size_after = fs::metadata(&plan.temp_output)
            .map_err(|e| format!("Converted file missing after metadata copy: {e}"))?
            .len();
    }

    if plan.media_type == BatchMediaType::Video
        && encoder.probe_duration(&plan.temp_output).is_none()
    {
        let _ = fs::remove_file(&plan.temp_output);
        return Err("Converted video could not be read back.".into());
    }

    let size_before = fs::metadata(&plan.input).map(|m| m.len()).unwrap_or(0);
    if let Some(reason) = savings_rejection(size_before, size_after, settings) {
        let _ = fs::remove_file(&plan.temp_output);
        return Ok(ItemOutcome::Skipped { reason });
    }

    if settings.preserve_timestamps {
        if let Ok(snapshot) = read_timestamps(&plan.input) {
            let _ = apply_timestamps(&plan.temp_output, &snapshot);
        }
    }

    if !plan.replace_original {
        fs::rename(&plan.temp_output, &plan.final_output)
            .map_err(|e| format!("Could not write {}: {e}", plan.final_output.display()))?;
        return Ok(ItemOutcome::Done {
            output_path: plan.final_output.to_string_lossy().to_string(),
            size_after,
        });
    }

    let backup_path = if plan.backup {
        let path = backup_path_for(&plan.input);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Cannot create backup folder: {e}"))?;
        }
        copy_file_preserve(&plan.input, &path)
            .map_err(|e| format!("Could not back up the original: {e}"))?;
        Some(path)
    } else {
        None
    };

    let target_backup_path = if let Some(target) = &plan.overwritten_target {
        let path = backup_path_for(target);
        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                if let Some(backup) = &backup_path {
                    let _ = fs::remove_file(backup);
                }
                return Err(format!("Cannot create target backup folder: {error}"));
            }
        }
        if let Err(error) = copy_file_preserve(target, &path) {
            if let Some(backup) = &backup_path {
                let _ = fs::remove_file(backup);
            }
            return Err(format!("Could not back up the existing output: {error}"));
        }
        Some(path)
    } else {
        None
    };

    // The original is moved aside, never deleted outright: until the converted
    // file is in place there has to be a copy on disk to put back. Deleting
    // first left a window where a failing rename lost the file for good.
    let displaced = displaced_path_for(&plan.input);
    if let Err(error) = fs::rename(&plan.input, &displaced) {
        let _ = fs::remove_file(&plan.temp_output);
        if let Some(backup) = &backup_path {
            let _ = fs::remove_file(backup);
        }
        if let Some(backup) = &target_backup_path {
            let _ = fs::remove_file(backup);
        }
        return Err(format!("Could not move the original aside: {error}"));
    }

    let displaced_target = if let Some(target) = &plan.overwritten_target {
        let displaced_target = displaced_path_for(target);
        if let Err(error) = fs::rename(target, &displaced_target) {
            let _ = fs::rename(&displaced, &plan.input);
            let _ = fs::remove_file(&plan.temp_output);
            if let Some(backup) = &backup_path {
                let _ = fs::remove_file(backup);
            }
            if let Some(backup) = &target_backup_path {
                let _ = fs::remove_file(backup);
            }
            return Err(format!("Could not move the existing output aside: {error}"));
        }
        Some(displaced_target)
    } else {
        None
    };

    if let Err(error) = fs::rename(&plan.temp_output, &plan.final_output) {
        // Put the original back before reporting the failure.
        if let (Some(target), Some(displaced_target)) =
            (&plan.overwritten_target, &displaced_target)
        {
            let _ = fs::rename(displaced_target, target);
        }
        let _ = fs::rename(&displaced, &plan.input);
        if let Some(backup) = &backup_path {
            let _ = fs::remove_file(backup);
        }
        if let Some(backup) = &target_backup_path {
            let _ = fs::remove_file(backup);
        }
        let _ = fs::remove_file(&plan.temp_output);
        return Err(format!("Could not replace the original: {error}"));
    }

    let _ = fs::remove_file(&displaced);
    if let Some(displaced_target) = &displaced_target {
        let _ = fs::remove_file(displaced_target);
    }

    if backup_path.is_some() || target_backup_path.is_some() {
        if let Ok(mut guard) = job.lock() {
            if let Some(backup) = backup_path {
                guard.replacements.push(BatchReplacement {
                    backup_path: backup.to_string_lossy().to_string(),
                    original_path: plan.input.to_string_lossy().to_string(),
                    converted_path: plan.final_output.to_string_lossy().to_string(),
                });
            }
            if let (Some(target), Some(backup)) = (&plan.overwritten_target, target_backup_path) {
                guard.replacements.push(BatchReplacement {
                    backup_path: backup.to_string_lossy().to_string(),
                    original_path: target.to_string_lossy().to_string(),
                    converted_path: target.to_string_lossy().to_string(),
                });
            }
        }
    }

    Ok(ItemOutcome::Done {
        output_path: plan.final_output.to_string_lossy().to_string(),
        size_after,
    })
}

fn savings_rejection(
    size_before: u64,
    size_after: u64,
    settings: &BatchSettings,
) -> Option<String> {
    if size_before == 0 {
        return None;
    }
    if settings.skip_if_larger && size_after >= size_before {
        return Some("Converted file was not smaller — original kept.".into());
    }
    if let Some(min_pct) = settings.skip_if_savings_below_pct {
        let saved_pct = 100.0 - (size_after as f64 * 100.0 / size_before as f64);
        if saved_pct < min_pct as f64 {
            return Some(format!(
                "Only {saved_pct:.0}% smaller (below the {min_pct}% threshold) — original kept."
            ));
        }
    }
    None
}

/// Hidden sibling that holds the original while its replacement is put in
/// place. Same folder, so the move is a rename and cannot half-succeed.
fn displaced_path_for(source: &Path) -> PathBuf {
    let dir = source.parent().unwrap_or_else(|| Path::new("."));
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("media");
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f");
    dir.join(format!(".qmo-replacing-{stamp}-{name}"))
}

pub fn backup_path_for(source: &Path) -> PathBuf {
    let dir = source
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(APP_FOLDER_NAME)
        .join("batch-backups");
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("media");
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f");
    dir.join(format!("{stem}_{stamp}.{ext}"))
}

fn update_item<F>(
    job: &Arc<Mutex<BatchJobStatus>>,
    index: usize,
    apply: F,
) -> Option<BatchItemStatus>
where
    F: FnOnce(&mut BatchItemStatus),
{
    let mut guard = job.lock().ok()?;
    let item = guard.items.get_mut(index)?;
    apply(item);
    Some(item.clone())
}

/// Counters are derived from the item that just reached a terminal state.
fn apply_terminal_state(job: &Arc<Mutex<BatchJobStatus>>, status: &BatchItemStatus) {
    let Ok(mut guard) = job.lock() else {
        return;
    };
    match status.state {
        BatchItemState::Done => {
            guard.done += 1;
            guard.bytes_before += status.size_before;
            guard.bytes_after += status.size_after.unwrap_or(status.size_before);
        }
        BatchItemState::Skipped => guard.skipped += 1,
        BatchItemState::Failed => guard.failed += 1,
        _ => {}
    }
}

fn recalculate_terminal_state(job: &Arc<Mutex<BatchJobStatus>>) {
    let Ok(mut guard) = job.lock() else {
        return;
    };
    let mut done = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut bytes_before = 0;
    let mut bytes_after = 0;
    for item in &guard.items {
        match item.state {
            BatchItemState::Done => {
                done += 1;
                bytes_before += item.size_before;
                bytes_after += item.size_after.unwrap_or(item.size_before);
            }
            BatchItemState::Failed => failed += 1,
            BatchItemState::Skipped => skipped += 1,
            _ => {}
        }
    }
    guard.done = done;
    guard.failed = failed;
    guard.skipped = skipped;
    guard.bytes_before = bytes_before;
    guard.bytes_after = bytes_after;
}

fn emit_progress<V: BatchEvents>(job: &Arc<Mutex<BatchJobStatus>>, events: &V) {
    let summary = job.lock().ok().map(|guard| guard.summary());
    if let Some(summary) = summary {
        events.progress(&summary);
    }
}

fn finish_job<V: BatchEvents>(job: &Arc<Mutex<BatchJobStatus>>, cancel: &AtomicBool, events: &V) {
    let snapshot = {
        let Ok(mut guard) = job.lock() else {
            return;
        };
        guard.running = false;
        guard.cancelled = cancel.load(Ordering::Relaxed);
        guard.finished_at = Some(chrono::Local::now().to_rfc3339());
        if guard.cancelled {
            for item in guard.items.iter_mut() {
                if matches!(
                    item.state,
                    BatchItemState::Pending | BatchItemState::Running
                ) {
                    item.state = BatchItemState::Cancelled;
                }
            }
        }
        guard.clone()
    };

    events.progress(&snapshot.summary());
    events.done(&snapshot);
}

pub fn new_job_status(
    job_id: String,
    items: Vec<BatchItemStatus>,
    output_dir: Option<String>,
    replaces_originals: bool,
) -> BatchJobStatus {
    BatchJobStatus {
        job_id,
        running: true,
        cancelled: false,
        total: items.len(),
        done: 0,
        failed: 0,
        skipped: 0,
        bytes_before: 0,
        bytes_after: 0,
        started_at: chrono::Local::now().to_rfc3339(),
        finished_at: None,
        output_dir,
        replaces_originals,
        items,
        replacements: Vec::new(),
        finalized: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch::{ConflictPolicy, ImageFormat, VideoCodec};
    use crate::state::AppState;
    use filetime::FileTime;
    use std::sync::atomic::AtomicU32;

    struct FakeEncoder {
        /// Bytes written for every produced file.
        output_size: usize,
        fail_on: Option<String>,
        /// Fails whenever the flags contain this fragment.
        fail_flag: Option<String>,
        fail_flag_error: Option<String>,
        calls: AtomicU32,
    }

    struct ConcurrencyEncoder {
        active: AtomicUsize,
        max_active: AtomicUsize,
    }

    impl MediaEncoder for ConcurrencyEncoder {
        fn probe_duration(&self, _path: &Path) -> Option<f64> {
            Some(1.0)
        }

        fn encode(&self, request: EncodeRequest<'_>) -> Result<(), String> {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            fs::write(request.output, vec![b'x'; 100]).map_err(|error| error.to_string())?;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        }
    }

    impl FakeEncoder {
        fn new(output_size: usize) -> Self {
            Self {
                output_size,
                fail_on: None,
                fail_flag: None,
                fail_flag_error: None,
                calls: AtomicU32::new(0),
            }
        }
    }

    impl MediaEncoder for FakeEncoder {
        fn probe_duration(&self, _path: &Path) -> Option<f64> {
            Some(10.0)
        }

        fn encode(&self, request: EncodeRequest<'_>) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if request.cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.to_string());
            }
            if let Some(needle) = &self.fail_on {
                if request.input.to_string_lossy().contains(needle.as_str()) {
                    return Err("boom".into());
                }
            }
            if let Some(needle) = &self.fail_flag {
                if request.output_flags.join(" ").contains(needle.as_str()) {
                    return Err(self.fail_flag_error.clone().unwrap_or_else(|| {
                        "Could not write header (incorrect codec parameters?)".into()
                    }));
                }
            }
            (request.on_progress)(0.5);
            fs::write(request.output, vec![b'x'; self.output_size]).map_err(|e| e.to_string())?;
            (request.on_progress)(1.0);
            Ok(())
        }
    }

    #[derive(Default)]
    struct CollectedEvents {
        items: Mutex<Vec<BatchItemStatus>>,
        done: Mutex<Option<BatchJobStatus>>,
    }

    impl BatchEvents for CollectedEvents {
        fn item(&self, item: &BatchItemStatus) {
            self.items.lock().unwrap().push(item.clone());
        }
        fn progress(&self, _summary: &BatchProgressSummary) {}
        fn done(&self, job: &BatchJobStatus) {
            *self.done.lock().unwrap() = Some(job.clone());
        }
    }

    fn write_source(dir: &Path, name: &str, size: usize) -> String {
        let path = dir.join(name);
        fs::write(&path, vec![b'a'; size]).unwrap();
        path.to_string_lossy().to_string()
    }

    fn image_settings(output: OutputMode) -> BatchSettings {
        BatchSettings {
            output,
            concurrency: 2,
            skip_if_savings_below_pct: None,
            ..BatchSettings::default()
        }
    }

    fn run(
        paths: Vec<String>,
        settings: BatchSettings,
        encoder: FakeEncoder,
        cancel: Arc<AtomicBool>,
    ) -> (BatchJobStatus, Arc<CollectedEvents>) {
        run_with_capabilities(paths, settings, encoder, cancel, &[software_capability()])
    }

    fn run_with_capabilities(
        paths: Vec<String>,
        settings: BatchSettings,
        encoder: FakeEncoder,
        cancel: Arc<AtomicBool>,
        capabilities: &[VideoBackendCapability],
    ) -> (BatchJobStatus, Arc<CollectedEvents>) {
        let plans = plan_items_with_capabilities(&paths, &settings, capabilities);
        let statuses: Vec<BatchItemStatus> =
            plans.iter().map(|planned| planned.status.clone()).collect();
        let job = Arc::new(Mutex::new(new_job_status(
            "job-test".into(),
            statuses,
            None,
            matches!(settings.output, OutputMode::ReplaceOriginal { .. }),
        )));
        let events = Arc::new(CollectedEvents::default());
        run_job(
            Arc::clone(&job),
            plans,
            settings,
            Arc::new(encoder),
            Arc::clone(&events),
            cancel,
        );
        let snapshot = job.lock().unwrap().clone();
        (snapshot, events)
    }

    #[test]
    fn converts_into_a_subfolder_and_aggregates_savings() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_source(dir.path(), "a.jpg", 1000);
        let b = write_source(dir.path(), "b.jpg", 1000);

        let (job, events) = run(
            vec![a, b],
            image_settings(OutputMode::Subfolder {
                name: "_optimized".into(),
            }),
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 2);
        assert_eq!(job.failed, 0);
        assert_eq!(job.bytes_before, 2000);
        assert_eq!(job.bytes_after, 200);
        assert!(!job.running);
        assert!(dir.path().join("_optimized/a.jpg").is_file());
        assert!(dir.path().join("_optimized/b.jpg").is_file());
        // Originals untouched.
        assert_eq!(fs::metadata(dir.path().join("a.jpg")).unwrap().len(), 1000);
        assert!(!events.items.lock().unwrap().is_empty());
        assert!(events.done.lock().unwrap().is_some());
    }

    #[test]
    fn keeps_the_original_when_the_result_is_not_smaller() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_source(dir.path(), "a.jpg", 100);

        let (job, _) = run(
            vec![a],
            image_settings(OutputMode::Subfolder {
                name: "_optimized".into(),
            }),
            FakeEncoder::new(500),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.skipped, 1);
        assert_eq!(job.done, 0);
        assert!(!dir.path().join("_optimized/a.jpg").exists());
        assert!(no_temp_files(dir.path()));
    }

    #[test]
    fn enforces_the_minimum_savings_threshold() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_source(dir.path(), "a.jpg", 1000);
        let mut settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        settings.skip_if_savings_below_pct = Some(50);

        let (job, _) = run(
            vec![a],
            settings,
            FakeEncoder::new(900),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.skipped, 1);
        assert!(job.items[0].error.as_deref().unwrap().contains("threshold"));
    }

    #[test]
    fn replacing_originals_backs_them_up_and_records_undo_pairs() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_source(dir.path(), "a.heic", 1000);

        let mut settings = image_settings(OutputMode::ReplaceOriginal {
            backup: true,
            confirmed: true,
        });
        settings.image.format = ImageFormat::Jpeg;

        let (job, _) = run(
            vec![a.clone()],
            settings,
            FakeEncoder::new(200),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert!(!Path::new(&a).exists(), "original was replaced");
        assert!(dir.path().join("a.jpg").is_file());
        assert_eq!(job.replacements.len(), 1);
        let replacement = &job.replacements[0];
        assert!(Path::new(&replacement.backup_path).is_file());
        assert_eq!(replacement.original_path, a);
        assert_ne!(replacement.converted_path, replacement.original_path);
    }

    #[test]
    fn replace_and_undo_restore_exact_bytes_and_timestamps() {
        let dir = tempfile::tempdir().unwrap();
        let app_data = tempfile::tempdir().unwrap();
        let source = write_source(dir.path(), "photo.jpg", 1000);
        let original_bytes = fs::read(&source).unwrap();
        let original_time = FileTime::from_unix_time(1_600_000_000, 0);

        let mut state = AppState::new(app_data.path().to_path_buf());
        state.open_folder(dir.path().to_path_buf()).unwrap();
        filetime::set_file_times(&source, original_time, original_time).unwrap();

        let settings = image_settings(OutputMode::ReplaceOriginal {
            backup: true,
            confirmed: true,
        });
        let (job, _) = run(
            vec![source.clone()],
            settings,
            FakeEncoder::new(200),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert_eq!(fs::metadata(&source).unwrap().len(), 200);
        let replacements: Vec<(String, String, String)> = job
            .replacements
            .iter()
            .map(|item| {
                (
                    item.backup_path.clone(),
                    item.original_path.clone(),
                    item.converted_path.clone(),
                )
            })
            .collect();
        let backup_times = read_timestamps(Path::new(&replacements[0].0)).unwrap();
        assert_eq!(backup_times.modified, original_time);
        assert_eq!(state.apply_batch_replacements(&replacements).unwrap(), 1);
        assert_eq!(state.apply_batch_replacements(&replacements).unwrap(), 0);
        assert_eq!(state.undo_stack.len(), 1);
        state.undo_last().unwrap();

        let restored = read_timestamps(Path::new(&source)).unwrap();
        assert_eq!(restored.modified, original_time);
        assert_eq!(fs::read(&source).unwrap(), original_bytes);
        assert!(no_temp_files(dir.path()));
    }

    #[test]
    fn replace_original_rename_keeps_an_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_source(dir.path(), "photo.heic", 1000);
        let existing = write_source(dir.path(), "photo.jpg", 700);

        let mut settings = image_settings(OutputMode::ReplaceOriginal {
            backup: true,
            confirmed: true,
        });
        settings.image.format = ImageFormat::Jpeg;
        settings.on_conflict = ConflictPolicy::Rename;

        let (job, _) = run(
            vec![source],
            settings,
            FakeEncoder::new(200),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert_eq!(fs::metadata(existing).unwrap().len(), 700);
        assert!(dir.path().join("photo (2).jpg").is_file());
    }

    #[test]
    fn replace_original_skip_keeps_both_files_when_target_exists() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_source(dir.path(), "photo.heic", 1000);
        let existing = write_source(dir.path(), "photo.jpg", 700);

        let mut settings = image_settings(OutputMode::ReplaceOriginal {
            backup: true,
            confirmed: true,
        });
        settings.image.format = ImageFormat::Jpeg;
        settings.on_conflict = ConflictPolicy::Skip;

        let (job, _) = run(
            vec![source.clone()],
            settings,
            FakeEncoder::new(200),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.skipped, 1);
        assert_eq!(fs::metadata(source).unwrap().len(), 1000);
        assert_eq!(fs::metadata(existing).unwrap().len(), 700);
    }

    #[test]
    fn replace_original_overwrite_preserves_the_displaced_target_for_undo() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_source(dir.path(), "photo.heic", 1000);
        let existing = write_source(dir.path(), "photo.jpg", 700);

        let mut settings = image_settings(OutputMode::ReplaceOriginal {
            backup: true,
            confirmed: true,
        });
        settings.image.format = ImageFormat::Jpeg;
        settings.on_conflict = ConflictPolicy::Overwrite;

        let (job, _) = run(
            vec![source],
            settings,
            FakeEncoder::new(200),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert_eq!(fs::metadata(existing).unwrap().len(), 200);
        assert_eq!(job.replacements.len(), 2);
        let target_backup = job
            .replacements
            .iter()
            .find(|replacement| replacement.original_path.ends_with("photo.jpg"))
            .expect("the overwritten target has its own undo record");
        assert_eq!(fs::metadata(&target_backup.backup_path).unwrap().len(), 700);
    }

    #[test]
    fn refuses_to_replace_without_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_source(dir.path(), "a.jpg", 1000);

        let (job, _) = run(
            vec![a.clone()],
            image_settings(OutputMode::ReplaceOriginal {
                backup: true,
                confirmed: false,
            }),
            FakeEncoder::new(200),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.failed, 1);
        assert_eq!(fs::metadata(&a).unwrap().len(), 1000);
        assert!(job.items[0].error.as_deref().unwrap().contains("confirmed"));
    }

    #[test]
    fn cancelling_leaves_pending_items_cancelled_and_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<String> = (0..4)
            .map(|n| write_source(dir.path(), &format!("f{n}.jpg"), 1000))
            .collect();

        let cancel = Arc::new(AtomicBool::new(true));
        let (job, _) = run(
            paths.clone(),
            image_settings(OutputMode::Subfolder {
                name: "_optimized".into(),
            }),
            FakeEncoder::new(100),
            cancel,
        );

        assert!(job.cancelled);
        assert_eq!(job.done, 0);
        assert!(job
            .items
            .iter()
            .all(|item| item.state == BatchItemState::Cancelled));
        for path in &paths {
            assert!(Path::new(path).is_file(), "originals survive a cancel");
        }
        assert!(no_temp_files(dir.path()));
    }

    #[test]
    fn a_failed_item_does_not_stop_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let a = write_source(dir.path(), "bad.jpg", 1000);
        let b = write_source(dir.path(), "good.jpg", 1000);

        let mut encoder = FakeEncoder::new(100);
        encoder.fail_on = Some("bad".into());

        let (job, _) = run(
            vec![a, b],
            image_settings(OutputMode::Subfolder {
                name: "_optimized".into(),
            }),
            encoder,
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.failed, 1);
        assert_eq!(job.done, 1);
        assert!(dir.path().join("_optimized/good.jpg").is_file());
        assert!(no_temp_files(dir.path()));
    }

    #[test]
    fn unsupported_files_fail_without_touching_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let notes = write_source(dir.path(), "notes.txt", 10);
        let missing = dir.path().join("gone.jpg").to_string_lossy().to_string();

        let (job, _) = run(
            vec![notes, missing],
            image_settings(OutputMode::Subfolder {
                name: "_optimized".into(),
            }),
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.failed, 2);
        assert!(!dir.path().join("_optimized").exists());
    }

    #[test]
    fn conflicting_outputs_get_unique_names() {
        let dir = tempfile::tempdir().unwrap();
        let sub_a = dir.path().join("a");
        let sub_b = dir.path().join("b");
        fs::create_dir_all(&sub_a).unwrap();
        fs::create_dir_all(&sub_b).unwrap();
        let a = write_source(&sub_a, "IMG_1.jpg", 1000);
        let b = write_source(&sub_b, "IMG_1.jpg", 1000);

        let mut settings = image_settings(OutputMode::CustomFolder {
            path: dir.path().join("out").to_string_lossy().to_string(),
        });
        settings.on_conflict = ConflictPolicy::Rename;

        let (job, _) = run(
            vec![a, b],
            settings,
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 2);
        assert!(dir.path().join("out/IMG_1.jpg").is_file());
        assert!(dir.path().join("out/IMG_1 (2).jpg").is_file());
    }

    #[test]
    fn duplicate_overwrite_outputs_fail_instead_of_racing() {
        let dir = tempfile::tempdir().unwrap();
        let sub_a = dir.path().join("a");
        let sub_b = dir.path().join("b");
        fs::create_dir_all(&sub_a).unwrap();
        fs::create_dir_all(&sub_b).unwrap();
        let a = write_source(&sub_a, "IMG_1.jpg", 1000);
        let b = write_source(&sub_b, "IMG_1.jpg", 1000);

        let mut settings = image_settings(OutputMode::CustomFolder {
            path: dir.path().join("out").to_string_lossy().to_string(),
        });
        settings.on_conflict = ConflictPolicy::Overwrite;

        let (job, _) = run(
            vec![a, b],
            settings,
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert_eq!(job.failed, 1);
        assert!(job.items[1]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("same output")));
    }

    #[test]
    fn video_plans_use_the_video_flags() {
        let dir = tempfile::tempdir().unwrap();
        let clip = write_source(dir.path(), "clip.mov", 1000);
        let mut settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        settings.video.codec = VideoCodec::H265;

        let plans = plan_items(&[clip], &settings);
        let plan = plans[0].plan.as_ref().unwrap();
        assert_eq!(plan.media_type, BatchMediaType::Video);
        assert!(plan.attempts[0].output_flags.join(" ").contains("libx265"));
        assert_eq!(plan.final_output.extension().unwrap(), "mp4");
    }

    #[test]
    fn files_already_in_the_output_folder_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        fs::create_dir_all(&out).unwrap();
        let source = write_source(dir.path(), "a.jpg", 1000);
        let previous_result = write_source(&out, "b.jpg", 1000);

        let (job, _) = run(
            vec![source, previous_result],
            image_settings(OutputMode::CustomFolder {
                path: out.to_string_lossy().to_string(),
            }),
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert_eq!(job.skipped, 1);
        assert!(job.items[1]
            .error
            .as_deref()
            .unwrap()
            .contains("output folder"));
    }

    #[test]
    fn audio_copy_failures_retry_with_aac() {
        let dir = tempfile::tempdir().unwrap();
        let clip = write_source(dir.path(), "clip.mkv", 1000);

        let mut settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        settings.video.audio = AudioMode::Copy;

        let mut encoder = FakeEncoder::new(100);
        encoder.fail_flag = Some("-c:a copy".into());

        let (job, _) = run(
            vec![clip],
            settings,
            encoder,
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1, "the AAC retry rescued the file");
        assert!(dir.path().join("_optimized/clip.mp4").is_file());
    }

    #[test]
    fn unavailable_gpu_during_encode_falls_back_to_cpu_and_reports_it() {
        let dir = tempfile::tempdir().unwrap();
        let clip = write_source(dir.path(), "clip.mov", 1000);
        let settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        let capabilities = [VideoBackendCapability {
            backend: VideoBackend::Nvidia,
            codecs: vec![VideoCodec::H265],
            available: true,
            reason: None,
        }];
        let mut encoder = FakeEncoder::new(100);
        encoder.fail_flag = Some("hevc_nvenc".into());
        encoder.fail_flag_error = Some("No capable devices found".into());

        let (job, _) = run_with_capabilities(
            vec![clip],
            settings,
            encoder,
            Arc::new(AtomicBool::new(false)),
            &capabilities,
        );

        assert_eq!(job.done, 1);
        assert_eq!(job.items[0].encoder_backend, Some(VideoBackend::Software));
        assert!(job.items[0]
            .fallback_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("NVIDIA NVENC")));
    }

    #[test]
    fn hardware_encodes_are_serialized_while_the_job_has_multiple_workers() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![
            write_source(dir.path(), "one.mov", 1000),
            write_source(dir.path(), "two.mov", 1000),
        ];
        let mut settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        settings.concurrency = 2;
        let capabilities = [VideoBackendCapability {
            backend: VideoBackend::Nvidia,
            codecs: vec![VideoCodec::H265],
            available: true,
            reason: None,
        }];
        let plans = plan_items_with_capabilities(&paths, &settings, &capabilities);
        let statuses = plans.iter().map(|item| item.status.clone()).collect();
        let job = Arc::new(Mutex::new(new_job_status(
            "hardware-concurrency".into(),
            statuses,
            None,
            false,
        )));
        let encoder = Arc::new(ConcurrencyEncoder {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        });

        run_job(
            Arc::clone(&job),
            plans,
            settings,
            Arc::clone(&encoder),
            Arc::new(CollectedEvents::default()),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.lock().unwrap().done, 2);
        assert_eq!(encoder.max_active.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn resumed_jobs_keep_completed_files_and_run_only_pending_plans() {
        let dir = tempfile::tempdir().unwrap();
        let paths = vec![
            write_source(dir.path(), "done.mov", 1000),
            write_source(dir.path(), "pending.mov", 1000),
        ];
        let settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        let mut plans = plan_items(&paths, &settings);
        let completed_output = plans[0].plan.as_ref().unwrap().final_output.clone();
        fs::write(&completed_output, vec![b'd'; 120]).unwrap();
        plans[0].status.state = BatchItemState::Done;
        plans[0].status.progress = 1.0;
        plans[0].status.size_after = Some(120);
        plans[0].plan = None;
        let statuses = plans.iter().map(|item| item.status.clone()).collect();
        let job = Arc::new(Mutex::new(new_job_status(
            "resumed".into(),
            statuses,
            None,
            false,
        )));

        run_job(
            Arc::clone(&job),
            plans,
            settings,
            Arc::new(FakeEncoder::new(100)),
            Arc::new(CollectedEvents::default()),
            Arc::new(AtomicBool::new(false)),
        );

        let snapshot = job.lock().unwrap().clone();
        assert_eq!(snapshot.done, 2);
        assert_eq!(fs::read(completed_output).unwrap(), vec![b'd'; 120]);
    }

    #[test]
    fn unicode_paths_survive_planning_and_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let source = write_source(dir.path(), "vídeo familiar 🎬.mov", 1000);

        let (job, _) = run(
            vec![source],
            image_settings(OutputMode::Subfolder {
                name: "salida ágil".into(),
            }),
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1);
        assert!(dir
            .path()
            .join("salida ágil/vídeo familiar 🎬.mp4")
            .is_file());
    }

    #[cfg(windows)]
    #[test]
    fn windows_paths_longer_than_260_characters_are_supported() {
        let dir = tempfile::tempdir().unwrap();
        let mut deep = dir.path().to_path_buf();
        while deep.to_string_lossy().len() < 275 {
            deep.push("a-very-long-camera-backup-segment");
        }
        fs::create_dir_all(&deep).unwrap();
        let source = write_source(&deep, "clip.mov", 1000);

        let (job, _) = run(
            vec![source],
            image_settings(OutputMode::Subfolder {
                name: "_optimized".into(),
            }),
            FakeEncoder::new(100),
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.done, 1, "{:?}", job.items[0].error);
        assert!(deep.join("_optimized/clip.mp4").is_file());
    }

    #[test]
    fn a_real_failure_is_not_retried_forever() {
        let dir = tempfile::tempdir().unwrap();
        let clip = write_source(dir.path(), "clip.mkv", 1000);

        let mut settings = image_settings(OutputMode::Subfolder {
            name: "_optimized".into(),
        });
        settings.video.audio = AudioMode::Copy;

        let mut encoder = FakeEncoder::new(100);
        encoder.fail_on = Some("clip".into());

        let (job, _) = run(
            vec![clip],
            settings,
            encoder,
            Arc::new(AtomicBool::new(false)),
        );

        assert_eq!(job.failed, 1);
        assert!(no_temp_files(dir.path()));
    }

    fn no_temp_files(dir: &Path) -> bool {
        fn walk(dir: &Path) -> bool {
            let Ok(entries) = fs::read_dir(dir) else {
                return true;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    if !walk(&path) {
                        return false;
                    }
                } else if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name.starts_with(".qmo-tmp-"))
                {
                    return false;
                }
            }
            true
        }
        walk(dir)
    }
}
