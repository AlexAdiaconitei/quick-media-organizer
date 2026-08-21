import type { BatchItemStatus, BatchJobStatus, FrontendState, MediaItem } from "./types";

export type ScreenshotMode =
  | "welcome"
  | "workspace"
  | "workspace-video"
  | "batch-select"
  | "batch-settings"
  | "batch-progress"
  | "batch-done";

const MODES: ScreenshotMode[] = [
  "welcome",
  "workspace",
  "workspace-video",
  "batch-select",
  "batch-settings",
  "batch-progress",
  "batch-done",
];

export function getScreenshotMode(): ScreenshotMode | null {
  if (typeof window === "undefined") return null;
  const value = new URLSearchParams(window.location.search).get("screenshot");
  return MODES.includes(value as ScreenshotMode) ? (value as ScreenshotMode) : null;
}

const demoPhotoItem: MediaItem = {
  id: "demo-sunset",
  paths: ["/demo-sunset.jpg"],
  file_name: "IMG_4521.heic",
  extension: "heic",
  exif_date: "2024-08-12T19:42:00",
  modified_at: "2024-08-12T19:42:00",
  size_bytes: 2_457_600,
  is_video: false,
  kind: "live_photo",
  width: 3024,
  height: 4032,
};

const demoVideoItem: MediaItem = {
  id: "demo-waves",
  paths: ["/demo-video.mp4"],
  file_name: "IMG_8834.mov",
  extension: "mov",
  exif_date: "2024-08-12T19:55:00",
  modified_at: "2024-08-12T19:55:00",
  size_bytes: 8_420_000,
  is_video: true,
  kind: "single",
  width: 1080,
  height: 1920,
};

function buildBaseWorkspaceState(item: MediaItem, index: number): FrontendState {
  return {
    folder_path: "/Users/demo/Phone Backup 2024",
    current_index: index,
    total: 2410,
    item,
    sort_mode: "exif_date",
    scan_recursive: false,
    rename_mode: "free",
    armed_folder: "trips/portugal/algarve/beach-holidays-2024",
    recent_folders: ["trips/portugal/algarve/beach-holidays-2024", "gym", "paperwork"],
    favorite_folders: ["trips/portugal/algarve/beach-holidays-2024"],
    existing_subfolders: [
      "gym",
      "trips",
      "trips/portugal",
      "trips/portugal/algarve",
      "trips/portugal/algarve/beach-holidays-2024",
      "paperwork",
    ],
    stats: { renamed: 412, trashed: 89, moved: 346, skipped: 1203 },
  };
}

export function buildScreenshotWorkspaceState(): FrontendState {
  return buildBaseWorkspaceState(demoPhotoItem, 846);
}

export function buildScreenshotVideoWorkspaceState(): FrontendState {
  return buildBaseWorkspaceState(demoVideoItem, 847);
}

// --- Batch panel -----------------------------------------------------------

const CLIP_NAMES = [
  "IMG_8834.mov",
  "IMG_8835.mov",
  "IMG_8871.mp4",
  "IMG_8902.mov",
  "VID_20240813_101122.mp4",
  "VID_20240813_144501.mp4",
];

const PHOTO_NAMES = [
  "IMG_4521.heic",
  "IMG_4522.heic",
  "IMG_4530.heic",
  "IMG_4544.jpg",
  "IMG_4560.heic",
  "IMG_4575.jpg",
];

/// A believable selection: a phone folder is mostly clips and stills of very
/// different weights.
export function buildScreenshotBatchItems(): MediaItem[] {
  const clips = CLIP_NAMES.map((file_name, index) => ({
    id: `demo-clip-${index}`,
    paths: ["/demo-video.mp4"],
    file_name,
    extension: file_name.split(".").pop() ?? "mov",
    exif_date: "2024-08-13T10:11:00",
    modified_at: "2024-08-13T10:11:00",
    size_bytes: 148_000_000 + index * 21_400_000,
    is_video: true,
    kind: "single" as const,
    width: 1080,
    height: 1920,
  }));

  const photos = PHOTO_NAMES.map((file_name, index) => ({
    id: `demo-photo-${index}`,
    paths: ["/demo-sunset.jpg"],
    file_name,
    extension: file_name.split(".").pop() ?? "heic",
    exif_date: "2024-08-12T19:42:00",
    modified_at: "2024-08-12T19:42:00",
    size_bytes: 2_100_000 + index * 380_000,
    is_video: false,
    kind: "single" as const,
    width: 3024,
    height: 4032,
  }));

  return [...clips, ...photos];
}

function batchItemStatus(
  item: MediaItem,
  state: BatchItemStatus["state"],
  progress: number,
  ratio: number,
): BatchItemStatus {
  return {
    id: item.id,
    source_path: `/Users/demo/Phone Backup 2024/${item.file_name}`,
    file_name: item.file_name,
    media_type: item.is_video ? "video" : "image",
    state,
    progress,
    size_before: item.size_bytes,
    size_after: state === "done" ? Math.round(item.size_bytes * ratio) : null,
    output_path: state === "done" ? `/Users/demo/Optimized/${item.file_name}` : null,
    error:
      state === "skipped"
        ? "Only 3% smaller (below the 5% threshold) — original kept."
        : null,
  };
}

/// Mid-run by default: some files converted, one encoding, the rest queued.
/// `finished` gives the completed job instead, with the savings summary.
export function buildScreenshotBatchJob(finished = false): BatchJobStatus {
  const items = buildScreenshotBatchItems();
  const statuses = items.map((item, index) => {
    if (finished) {
      return index === 5
        ? batchItemStatus(item, "skipped", 1, 1)
        : batchItemStatus(item, "done", 1, item.is_video ? 0.26 : 0.42);
    }
    if (index < 4) return batchItemStatus(item, "done", 1, 0.26);
    if (index === 4) return batchItemStatus(item, "running", 0.62, 1);
    if (index === 5) return batchItemStatus(item, "skipped", 1, 1);
    return batchItemStatus(item, "pending", 0, 1);
  });

  const done = statuses.filter((status) => status.state === "done");

  return {
    job_id: "demo-job",
    running: !finished,
    cancelled: false,
    total: statuses.length,
    done: done.length,
    failed: 0,
    skipped: 1,
    bytes_before: done.reduce((sum, status) => sum + status.size_before, 0),
    bytes_after: done.reduce((sum, status) => sum + (status.size_after ?? 0), 0),
    started_at: "2024-08-13T10:15:00Z",
    finished_at: finished ? "2024-08-13T10:41:00Z" : null,
    output_dir: "/Users/demo/Optimized",
    replaces_originals: false,
    items: statuses,
    replacements: [],
    finalized: false,
  };
}
