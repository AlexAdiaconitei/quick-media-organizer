pub mod ffmpeg_args;
#[cfg(test)]
mod ffmpeg_smoke;
pub mod runner;

use serde::{Deserialize, Serialize};

pub use runner::{BatchRunner, SharedBatchState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchMediaType {
    Video,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    H264,
    H265,
    Av1,
    /// Stream copy: only remux (fast, lossless, small savings).
    Copy,
}

impl Default for VideoCodec {
    fn default() -> Self {
        Self::H265
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Jpeg,
    Webp,
    Avif,
    Png,
    /// Keep the source format, only re-encode/resize.
    Keep,
}

impl Default for ImageFormat {
    fn default() -> Self {
        Self::Jpeg
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMode {
    Copy,
    Aac,
    Drop,
}

impl Default for AudioMode {
    fn default() -> Self {
        Self::Aac
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictPolicy {
    Skip,
    Rename,
    Overwrite,
}

impl Default for ConflictPolicy {
    fn default() -> Self {
        Self::Rename
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum OutputMode {
    /// Write next to the source, inside `<source dir>/<name>`.
    Subfolder {
        #[serde(default = "default_subfolder")]
        name: String,
    },
    /// Write every result into one folder chosen by the user.
    CustomFolder { path: String },
    /// Destructive: replace the source file. Requires an explicit confirmation
    /// from the UI dialog; the backend refuses `confirmed: false`.
    ReplaceOriginal {
        #[serde(default = "default_true")]
        backup: bool,
        #[serde(default)]
        confirmed: bool,
    },
}

fn default_subfolder() -> String {
    "_optimized".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for OutputMode {
    fn default() -> Self {
        Self::Subfolder {
            name: default_subfolder(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoSettings {
    #[serde(default)]
    pub codec: VideoCodec,
    #[serde(default = "default_crf")]
    pub crf: u8,
    #[serde(default = "default_speed_preset")]
    pub speed_preset: String,
    #[serde(default)]
    pub max_height: Option<u32>,
    #[serde(default)]
    pub max_fps: Option<u32>,
    #[serde(default)]
    pub audio: AudioMode,
    #[serde(default = "default_audio_bitrate")]
    pub audio_bitrate_kbps: u32,
    #[serde(default = "default_true")]
    pub faststart: bool,
    #[serde(default = "default_true")]
    pub keep_metadata: bool,
}

fn default_crf() -> u8 {
    28
}

fn default_speed_preset() -> String {
    "medium".to_string()
}

fn default_audio_bitrate() -> u32 {
    128
}

impl Default for VideoSettings {
    fn default() -> Self {
        Self {
            codec: VideoCodec::default(),
            crf: default_crf(),
            speed_preset: default_speed_preset(),
            max_height: Some(1080),
            max_fps: None,
            audio: AudioMode::default(),
            audio_bitrate_kbps: default_audio_bitrate(),
            faststart: true,
            keep_metadata: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageSettings {
    #[serde(default)]
    pub format: ImageFormat,
    #[serde(default = "default_image_quality")]
    pub quality: u8,
    #[serde(default)]
    pub max_edge: Option<u32>,
    #[serde(default = "default_true")]
    pub keep_metadata: bool,
}

fn default_image_quality() -> u8 {
    85
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            format: ImageFormat::default(),
            quality: default_image_quality(),
            max_edge: None,
            keep_metadata: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchSettings {
    #[serde(default)]
    pub video: VideoSettings,
    #[serde(default)]
    pub image: ImageSettings,
    #[serde(default)]
    pub output: OutputMode,
    #[serde(default)]
    pub name_suffix: Option<String>,
    #[serde(default)]
    pub on_conflict: ConflictPolicy,
    #[serde(default = "default_true")]
    pub skip_if_larger: bool,
    #[serde(default = "default_min_savings")]
    pub skip_if_savings_below_pct: Option<u8>,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    #[serde(default = "default_true")]
    pub preserve_timestamps: bool,
}

fn default_min_savings() -> Option<u8> {
    Some(5)
}

pub fn default_concurrency() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(1)
        .min(8)
}

impl Default for BatchSettings {
    fn default() -> Self {
        Self {
            video: VideoSettings::default(),
            image: ImageSettings::default(),
            output: OutputMode::default(),
            name_suffix: None,
            on_conflict: ConflictPolicy::default(),
            skip_if_larger: true,
            skip_if_savings_below_pct: default_min_savings(),
            concurrency: default_concurrency(),
            preserve_timestamps: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchItemState {
    Pending,
    Running,
    Done,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItemStatus {
    pub id: String,
    pub source_path: String,
    pub file_name: String,
    pub media_type: BatchMediaType,
    pub state: BatchItemState,
    pub progress: f32,
    pub size_before: u64,
    pub size_after: Option<u64>,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

/// One in-place replacement, kept so the session can register it for undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchReplacement {
    /// Backup copy of the original file.
    pub backup_path: String,
    /// Where the original lived (undo restores the backup here).
    pub original_path: String,
    /// File written by the conversion; undo removes it when it differs from
    /// `original_path` (format changed, e.g. .heic -> .jpg).
    pub converted_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJobStatus {
    pub job_id: String,
    pub running: bool,
    pub cancelled: bool,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub output_dir: Option<String>,
    pub replaces_originals: bool,
    pub items: Vec<BatchItemStatus>,
    #[serde(default)]
    pub replacements: Vec<BatchReplacement>,
    #[serde(default)]
    pub finalized: bool,
}

impl BatchJobStatus {
    /// Lightweight snapshot for the frequent `batch://progress` event.
    pub fn summary(&self) -> BatchProgressSummary {
        BatchProgressSummary {
            job_id: self.job_id.clone(),
            running: self.running,
            cancelled: self.cancelled,
            total: self.total,
            done: self.done,
            failed: self.failed,
            skipped: self.skipped,
            bytes_before: self.bytes_before,
            bytes_after: self.bytes_after,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProgressSummary {
    pub job_id: String,
    pub running: bool,
    pub cancelled: bool,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub skipped: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPreset {
    pub id: String,
    pub name: String,
    pub settings: BatchSettings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FfmpegCapabilities {
    pub available: bool,
    pub h264: bool,
    pub h265: bool,
    pub av1: bool,
    pub webp: bool,
    pub avif: bool,
    pub heic_decode: bool,
    pub version: Option<String>,
}
