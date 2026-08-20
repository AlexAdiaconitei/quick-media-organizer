use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    ExifDate,
    FileName,
    ModifiedDate,
}

impl Default for SortMode {
    fn default() -> Self {
        Self::ExifDate
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenameMode {
    Free,
    PrefixCounter,
}

impl Default for RenameMode {
    fn default() -> Self {
        Self::Free
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    Sidebar,
    Bottom,
}

impl Default for LayoutMode {
    fn default() -> Self {
        Self::Sidebar
    }
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
pub struct MediaMetadata {
    pub file_name: String,
    pub exif_date: Option<String>,
    pub modified_at: Option<String>,
    pub size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub is_video: bool,
    pub kind: MediaKind,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
    pub state: FrontendState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub locale: String,
    pub first_run_completed: bool,
    pub favorite_folders: Vec<String>,
    #[serde(default)]
    pub layout_mode: LayoutMode,
    #[serde(default = "default_show_metadata")]
    pub show_metadata: bool,
    #[serde(default)]
    pub video_with_sound: bool,
    #[serde(default)]
    pub last_folder_path: Option<String>,
    #[serde(default)]
    pub batch_presets: Vec<crate::batch::BatchPreset>,
    #[serde(default)]
    pub last_batch_settings: Option<crate::batch::BatchSettings>,
}

fn default_show_metadata() -> bool {
    true
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
    FlattenToRoot {
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
    /// backup over its original path; `remove_paths` holds converted files that
    /// changed extension (.heic -> .jpg) and must be deleted first.
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
