export type SortMode = "exif_date" | "file_name" | "modified_date";
export type RenameMode = "free" | "prefix_counter";
export type LayoutMode = "sidebar" | "bottom";
export type MediaKind = "single" | "live_photo";

export interface MediaItem {
  id: string;
  paths: string[];
  file_name: string;
  extension: string;
  exif_date?: string | null;
  modified_at?: string | null;
  size_bytes: number;
  is_video: boolean;
  kind: MediaKind;
  width?: number | null;
  height?: number | null;
}

export interface SessionStats {
  renamed: number;
  trashed: number;
  moved: number;
  skipped: number;
}

export interface FrontendState {
  folder_path?: string | null;
  current_index: number;
  total: number;
  item?: MediaItem | null;
  sort_mode: SortMode;
  scan_recursive: boolean;
  rename_mode: RenameMode;
  armed_folder?: string | null;
  recent_folders: string[];
  favorite_folders: string[];
  existing_subfolders: string[];
  stats: SessionStats;
  session_complete?: boolean;
  session_reset?: boolean;
  resume_from?: number | null;
  subfolder_media_count?: number | null;
}

export interface ActionResult {
  success: boolean;
  /// Names a message in i18n.ts; the backend never sends wording.
  message_key: string;
  message_args: Record<string, string>;
  undo_history_trimmed: boolean;
  state: FrontendState;
}

export interface AppSettings {
  locale: string;
  first_run_completed: boolean;
  favorite_folders: string[];
  layout_mode?: LayoutMode;
  show_metadata?: boolean;
  video_with_sound?: boolean;
  last_folder_path?: string | null;
}

export type Locale = "en" | "es";

export type VideoPreviewMode = "native" | "proxy" | "unavailable";

export interface VideoPreviewInfo {
  playback_path: string;
  poster_path?: string | null;
  mode: VideoPreviewMode;
  hint?: string | null;
}

export interface MediaFileDiagnosis {
  issue: "empty" | "too_small" | "content_mismatch" | "unknown";
  size_bytes: number;
}

// --- Batch optimization / conversion -------------------------------------

export type BatchMediaType = "video" | "image";
export type VideoCodec = "h264" | "h265" | "av1" | "copy";
export type ImageFormat = "jpeg" | "webp" | "avif" | "png" | "keep";
export type AudioMode = "copy" | "aac" | "drop";
export type ConflictPolicy = "skip" | "rename" | "overwrite";
export type BatchItemState =
  | "pending"
  | "running"
  | "done"
  | "skipped"
  | "failed"
  | "cancelled";

export interface VideoSettings {
  codec: VideoCodec;
  crf: number;
  speed_preset: string;
  max_height?: number | null;
  max_fps?: number | null;
  audio: AudioMode;
  audio_bitrate_kbps: number;
  faststart: boolean;
  keep_metadata: boolean;
}

export interface ImageConvertSettings {
  format: ImageFormat;
  quality: number;
  max_edge?: number | null;
  keep_metadata: boolean;
}

export type OutputMode =
  | { mode: "subfolder"; name: string }
  | { mode: "custom_folder"; path: string }
  | { mode: "replace_original"; backup: boolean; confirmed: boolean };

export interface BatchSettings {
  video: VideoSettings;
  image: ImageConvertSettings;
  output: OutputMode;
  name_suffix?: string | null;
  on_conflict: ConflictPolicy;
  skip_if_larger: boolean;
  skip_if_savings_below_pct?: number | null;
  concurrency: number;
  preserve_timestamps: boolean;
}

export interface BatchItemStatus {
  id: string;
  source_path: string;
  file_name: string;
  media_type: BatchMediaType;
  state: BatchItemState;
  progress: number;
  size_before: number;
  size_after?: number | null;
  output_path?: string | null;
  error?: string | null;
}

export interface BatchReplacement {
  backup_path: string;
  original_path: string;
  converted_path: string;
}

export interface BatchJobStatus {
  job_id: string;
  running: boolean;
  cancelled: boolean;
  total: number;
  done: number;
  failed: number;
  skipped: number;
  bytes_before: number;
  bytes_after: number;
  started_at: string;
  finished_at?: string | null;
  output_dir?: string | null;
  replaces_originals: boolean;
  items: BatchItemStatus[];
  replacements: BatchReplacement[];
  finalized: boolean;
}

export interface BatchProgressSummary {
  job_id: string;
  running: boolean;
  cancelled: boolean;
  total: number;
  done: number;
  failed: number;
  skipped: number;
  bytes_before: number;
  bytes_after: number;
}

export interface BatchPreset {
  id: string;
  name: string;
  settings: BatchSettings;
}

export interface FfmpegCapabilities {
  available: boolean;
  h264: boolean;
  h265: boolean;
  av1: boolean;
  webp: boolean;
  avif: boolean;
  heic_decode: boolean;
  version?: string | null;
}
