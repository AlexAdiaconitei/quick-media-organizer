use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SortMode {
    #[default]
    ExifDate,
    FileName,
    ModifiedDate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RenameMode {
    #[default]
    Free,
    PrefixCounter,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LayoutMode {
    #[default]
    Sidebar,
    Bottom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Single,
    LivePhoto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: String,
    pub paths: Vec<String>,
    pub file_name: String,
    pub extension: String,
    pub exif_date: Option<String>,
    pub modified_at: Option<String>,
    pub size_bytes: u64,
    pub is_video: bool,
    pub kind: MediaKind,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPreviewInfo {
    pub playback_path: String,
    pub poster_path: Option<String>,
    #[serde(rename = "mode")]
    pub preview_mode: VideoPreviewMode,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VideoPreviewMode {
    Native,
    Proxy,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFileDiagnosis {
    pub issue: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendState {
    pub folder_path: Option<String>,
    pub current_index: usize,
    pub total: usize,
    pub item: Option<MediaItem>,
    pub sort_mode: SortMode,
    pub scan_recursive: bool,
    pub rename_mode: RenameMode,
    pub armed_folder: Option<String>,
    pub recent_folders: Vec<String>,
    pub favorite_folders: Vec<String>,
    pub existing_subfolders: Vec<String>,
    pub stats: SessionStats,
    #[serde(default)]
    pub session_complete: bool,
    /// True when a saved session position could not be restored (e.g. file renamed elsewhere).
    #[serde(default)]
    pub session_reset: bool,
    /// 1-based queue position when reopening a folder mid-session.
    #[serde(default)]
    pub resume_from: Option<usize>,
    /// Media files found in subfolders while scan_recursive is off.
    #[serde(default)]
    pub subfolder_media_count: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionStats {
    pub renamed: u32,
    pub trashed: u32,
    pub moved: u32,
    pub skipped: u32,
}

/// Outcome of a user action. The backend never builds a user-facing sentence:
/// it names a message and its placeholders, and the UI renders that in the
/// user's language (see `src/lib/i18n.ts`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub message_key: String,
    #[serde(default)]
    pub message_args: std::collections::HashMap<String, String>,
    /// The oldest undo entries were dropped to stay under the cap, and the UI
    /// should say so alongside the message.
    #[serde(default)]
    pub undo_history_trimmed: bool,
    pub state: FrontendState,
}

/// Every field defaults on its own, and the ones holding structured data are
/// read leniently. A single value this build cannot understand must never cost
/// the user the whole file: that is what made the first-run welcome screen
/// reappear and favourites vanish after an update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default)]
    pub first_run_completed: bool,
    #[serde(default, deserialize_with = "lenient")]
    pub favorite_folders: Vec<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub layout_mode: LayoutMode,
    #[serde(default = "default_show_metadata")]
    pub show_metadata: bool,
    #[serde(default)]
    pub video_with_sound: bool,
    #[serde(default)]
    pub last_folder_path: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub batch_presets: Vec<crate::batch::BatchPreset>,
    #[serde(default, deserialize_with = "lenient")]
    pub last_batch_settings: Option<crate::batch::BatchSettings>,
}

fn default_show_metadata() -> bool {
    true
}

fn default_locale() -> String {
    "en".to_string()
}

/// Reads a field, or falls back to its default when the stored value no longer
/// fits the type. Used for the settings that carry nested structures, where a
/// renamed variant or a dropped field would otherwise abort the whole parse.
fn lenient<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).unwrap_or_default())
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            locale: "en".to_string(),
            first_run_completed: false,
            favorite_folders: Vec::new(),
            layout_mode: LayoutMode::default(),
            show_metadata: true,
            video_with_sound: false,
            last_folder_path: None,
            batch_presets: Vec::new(),
            last_batch_settings: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UndoAction {
    Rename {
        moves: Vec<PathPair>,
        #[serde(default)]
        focus_paths: Vec<String>,
        #[serde(default)]
        stat_kind: UndoStatKind,
    },
    Trash {
        moves: Vec<PathPair>,
        #[serde(default)]
        focus_paths: Vec<String>,
        #[serde(default)]
        stat_kind: UndoStatKind,
    },
    MoveToFolder {
        moves: Vec<PathPair>,
        #[serde(default)]
        focus_paths: Vec<String>,
        #[serde(default)]
        stat_kind: UndoStatKind,
    },
    /// Written by the "gather to root" feature removed in 0.1.5. Never created
    /// any more, but sessions saved by an older build still carry these, so the
    /// variant stays deserializable. `AppState::open_folder` drops them from
    /// the stack and takes their moves back out of the counter they inflated.
    #[serde(rename = "flatten_to_root")]
    LegacyFlattenToRoot {
        moves: Vec<PathPair>,
        #[serde(default)]
        focus_paths: Vec<String>,
        #[serde(default)]
        stat_kind: UndoStatKind,
    },
    TrimVideo {
        moves: Vec<PathPair>,
        #[serde(default)]
        focus_paths: Vec<String>,
        #[serde(default)]
        stat_kind: UndoStatKind,
    },
    /// Batch conversion that replaced originals in place. `moves` restores each
    /// backup over its original path; `remove_paths` holds every converted file
    /// that must be moved aside before the backups can be restored.
    ConvertMedia {
        moves: Vec<PathPair>,
        #[serde(default)]
        focus_paths: Vec<String>,
        #[serde(default)]
        stat_kind: UndoStatKind,
        #[serde(default)]
        remove_paths: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UndoStatKind {
    #[default]
    None,
    Renamed,
    Trashed,
    Moved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPair {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub folder_path: String,
    pub current_index: usize,
    #[serde(default)]
    pub current_item_paths: Vec<String>,
    pub sort_mode: SortMode,
    pub scan_recursive: bool,
    pub rename_mode: RenameMode,
    pub counter_map: std::collections::HashMap<String, u32>,
    pub recent_folders: Vec<String>,
    pub armed_folder: Option<String>,
    pub undo_stack: Vec<UndoAction>,
    pub stats: SessionStats,
    #[serde(default)]
    pub processed_paths: Vec<String>,
}
