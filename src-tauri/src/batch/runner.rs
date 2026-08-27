//! Batch job runner.
//!
//! Deliberately independent from `AppState`: encoding a folder takes minutes,
//! and holding the app-state mutex for that long would freeze every other
//! command. Workers only touch the job's own mutex, for microseconds at a time.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::ffmpeg_args::{
    classify_path, image_flags, output_extension, resolve_output_path, temp_output_path,
    video_flags,
};
use super::{
    AudioMode, BatchItemState, BatchItemStatus, BatchJobStatus, BatchMediaType, BatchProgressSummary,
    BatchReplacement, BatchSettings, OutputMode,
};
use crate::fs_util::{apply_timestamps, read_timestamps};
use crate::path_util::{is_path_inside_root, APP_FOLDER_NAME};
use crate::video::{FfmpegTools, CANCELLED};

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

    fn encode(
        &self,
        input: &Path,
        output: &Path,
        flags: &[String],
        duration: Option<f64>,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(f32),
    ) -> Result<(), String>;
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
}

impl MediaEncoder for FfmpegEncoder {
    fn probe_duration(&self, path: &Path) -> Option<f64> {
        self.tools.probe_duration(path).ok()
    }

    fn encode(
        &self,
        input: &Path,
        output: &Path,
        flags: &[String],
        duration: Option<f64>,
        cancel: &AtomicBool,
        on_progress: &dyn Fn(f32),
    ) -> Result<(), String> {
        self.tools
            .encode(input, output, flags, duration, cancel, on_progress)
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
#[derive(Debug, Clone)]
pub struct EncodePlan {
    pub input: PathBuf,
    pub temp_output: PathBuf,
    pub final_output: PathBuf,
    pub flags: Vec<String>,
    /// Retried once with these when the first attempt fails (audio copy).
    pub fallback_flags: Option<Vec<String>>,
    pub media_type: BatchMediaType,
    pub replace_original: bool,
    pub backup: bool,
}

pub struct PlannedItem {
    pub status: BatchItemStatus,
    pub plan: Option<EncodePlan>,
}

/// Builds the work list. Unsupported or unreadable files come back already
/// marked as failed/skipped instead of blowing up the whole job.
pub fn plan_items(paths: &[String], settings: &BatchSettings) -> Vec<PlannedItem> {
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

        match build_plan(&source, media_type, settings, index, &reserved) {
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
) -> Result<PlanOutcome, String> {
    let ext = output_extension(media_type, settings, source)?;
    let flags = match media_type {
        BatchMediaType::Video => video_flags(&settings.video),
        BatchMediaType::Image => image_flags(&settings.image, &ext)?,
    };
    // Copying the audio stream fails whenever the source codec cannot live in
    // the target container (PCM in .avi, Vorbis in .mkv). Keep an AAC variant
    // ready so one bad stream does not lose the whole file.
    let fallback_flags = match media_type {
        BatchMediaType::Video if settings.video.audio == AudioMode::Copy => {
            let mut retry = settings.video.clone();
            retry.audio = AudioMode::Aac;
            Some(video_flags(&retry))
        }
        _ => None,
    };

    let source_dir = source
        .parent()
        .ok_or_else(|| "File has no parent folder.".to_string())?
        .to_path_buf();

    let (final_output, replace_original, backup) = match &settings.output {
        OutputMode::ReplaceOriginal { backup, confirmed } => {
            if !confirmed {
                return Err(
                    "Replacing originals was not confirmed. Re-open the confirmation dialog."
                        .into(),
                );
            }
            (source.with_extension(&ext), true, *backup)
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
                return Ok(PlanOutcome::Skip(
                    "A converted file already exists.".into(),
                ));
            };
            (resolved, false, false)
        }
    };

    Ok(PlanOutcome::Work(Box::new(EncodePlan {
        input: source.to_path_buf(),
        temp_output: temp_output_path(&final_output, index),
        final_output,
        flags,
        fallback_flags,
        media_type,
        replace_original,
        backup,
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

    // Items that never got a plan already carry their final state.
    for planned in &plans {
        if planned.plan.is_none() {
            let status = planned.status.clone();
            apply_terminal_state(&job, &status);
            events.item(&status);
        }
    }
    emit_progress(&job, events.as_ref());

    let next = Arc::new(AtomicUsize::new(0));
    let workers = settings.concurrency.clamp(1, 8).min(work.len().max(1));
    let work = Arc::new(work);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let next = Arc::clone(&next);
            let work = Arc::clone(&work);
            let job = Arc::clone(&job);
            let encoder = Arc::clone(&encoder);
            let events = Arc::clone(&events);
            let cancel = Arc::clone(&cancel);
            let settings = settings.clone();

            scope.spawn(move || loop {
                let slot = next.fetch_add(1, Ordering::Relaxed);
                if slot >= work.len() || cancel.load(Ordering::Relaxed) {
                    break;
                }
                let (index, plan) = &work[slot];
                process_one(
                    *index,
                    plan,
                    &settings,
                    &job,
                    encoder.as_ref(),
                    events.as_ref(),
                    &cancel,
                );
            });
        }
    });

    finish_job(&job, &cancel, events.as_ref());
}

fn process_one<E: MediaEncoder, V: BatchEvents>(
    index: usize,
    plan: &EncodePlan,
    settings: &BatchSettings,
    job: &Arc<Mutex<BatchJobStatus>>,
    encoder: &E,
    events: &V,
    cancel: &AtomicBool,
) {
    if cancel.load(Ordering::Relaxed) {
        return;
    }

    if let Some(status) = update_item(job, index, |item| {
        item.state = BatchItemState::Running;
        item.progress = 0.0;
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

    let mut attempt = encoder.encode(
        &plan.input,
        &plan.temp_output,
        &plan.flags,
        duration,
        cancel,
        &on_progress,
    );

    // One retry with re-encoded audio: stream copy fails whenever the source
    // audio codec cannot be stored in the target container.
    if let (Err(error), Some(fallback)) = (&attempt, &plan.fallback_flags) {
        if error != CANCELLED && !cancel.load(Ordering::Relaxed) {
            let _ = fs::remove_file(&plan.temp_output);
            attempt = encoder.encode(
                &plan.input,
                &plan.temp_output,
                fallback,
                duration,
                cancel,
                &on_progress,
            );
        }
    }

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
            })
        }
    };

    if let Some(status) = status {
        apply_terminal_state(job, &status);
        events.item(&status);
        emit_progress(job, events);
    }
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
    let size_after = fs::metadata(&plan.temp_output)
        .map_err(|e| format!("Converted file missing: {e}"))?
        .len();
    if size_after == 0 {
        let _ = fs::remove_file(&plan.temp_output);
        return Err("Converted file is empty.".into());
    }

    // ffmpeg writes no EXIF for stills, so the block is carried over by hand.
    // Done before the size rules so the saving reported to the user counts it,
    // and never fatal: losing the tags is not worth losing the conversion.
    if plan.media_type == BatchMediaType::Image && settings.image.keep_metadata {
        if let Err(error) = crate::metadata::copy_exif(&plan.input, &plan.temp_output) {
            eprintln!("[QMO][batch] could not carry EXIF over: {error}");
        }
    }

    if plan.media_type == BatchMediaType::Video && encoder.probe_duration(&plan.temp_output).is_none()
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
        fs::copy(&plan.input, &path).map_err(|e| format!("Could not back up the original: {e}"))?;
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
        return Err(format!("Could not move the original aside: {error}"));
    }

    if let Err(error) = fs::rename(&plan.temp_output, &plan.final_output) {
        // Put the original back before reporting the failure.
        let _ = fs::rename(&displaced, &plan.input);
        if let Some(backup) = &backup_path {
            let _ = fs::remove_file(backup);
        }
        let _ = fs::remove_file(&plan.temp_output);
        return Err(format!("Could not replace the original: {error}"));
    }

    let _ = fs::remove_file(&displaced);

    if let Some(backup) = backup_path {
        if let Ok(mut guard) = job.lock() {
            guard.replacements.push(BatchReplacement {
                backup_path: backup.to_string_lossy().to_string(),
                original_path: plan.input.to_string_lossy().to_string(),
                converted_path: plan.final_output.to_string_lossy().to_string(),
            });
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
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
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

fn emit_progress<V: BatchEvents>(job: &Arc<Mutex<BatchJobStatus>>, events: &V) {
    let summary = job.lock().ok().map(|guard| guard.summary());
    if let Some(summary) = summary {
        events.progress(&summary);
    }
}

fn finish_job<V: BatchEvents>(
    job: &Arc<Mutex<BatchJobStatus>>,
    cancel: &AtomicBool,
    events: &V,
) {
    let snapshot = {
        let Ok(mut guard) = job.lock() else {
            return;
        };
        guard.running = false;
        guard.cancelled = cancel.load(Ordering::Relaxed);
        guard.finished_at = Some(chrono::Local::now().to_rfc3339());
        if guard.cancelled {
            for item in guard.items.iter_mut() {
                if matches!(item.state, BatchItemState::Pending | BatchItemState::Running) {
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
    use std::sync::atomic::AtomicU32;

    struct FakeEncoder {
        /// Bytes written for every produced file.
        output_size: usize,
        fail_on: Option<String>,
        /// Fails whenever the flags contain this fragment.
        fail_flag: Option<String>,
        calls: AtomicU32,
    }

    impl FakeEncoder {
        fn new(output_size: usize) -> Self {
            Self {
                output_size,
                fail_on: None,
                fail_flag: None,
                calls: AtomicU32::new(0),
            }
        }
    }

    impl MediaEncoder for FakeEncoder {
        fn probe_duration(&self, _path: &Path) -> Option<f64> {
            Some(10.0)
        }

        fn encode(
            &self,
            input: &Path,
            output: &Path,
            flags: &[String],
            _duration: Option<f64>,
            cancel: &AtomicBool,
            on_progress: &dyn Fn(f32),
        ) -> Result<(), String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.to_string());
            }
            if let Some(needle) = &self.fail_on {
                if input.to_string_lossy().contains(needle.as_str()) {
                    return Err("boom".into());
                }
            }
            if let Some(needle) = &self.fail_flag {
                if flags.join(" ").contains(needle.as_str()) {
                    return Err("Could not write header (incorrect codec parameters?)".into());
                }
            }
            on_progress(0.5);
            fs::write(output, vec![b'x'; self.output_size]).map_err(|e| e.to_string())?;
            on_progress(1.0);
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
        let plans = plan_items(&paths, &settings);
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
        assert!(job.items[0]
            .error
            .as_deref()
            .unwrap()
            .contains("threshold"));
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
        assert!(plan.flags.join(" ").contains("libx265"));
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
