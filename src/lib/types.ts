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
  message: string;
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
