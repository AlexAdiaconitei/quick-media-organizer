import { invokeLogged } from "./errorReporter";
import { t, type Locale } from "./i18n";
import type {
  BatchJobStatus,
  BatchPreset,
  BatchSettings,
  FfmpegCapabilities,
  MediaItem,
} from "./types";

/// False in a plain browser (`pnpm dev` without the Tauri shell): there is no
/// IPC bridge, so invoke/listen would throw on every call.
export function isTauriAvailable(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function defaultBatchSettings(): BatchSettings {
  return {
    video: {
      codec: "h265",
      crf: 28,
      speed_preset: "medium",
      max_height: 1080,
      max_fps: null,
      audio: "aac",
      audio_bitrate_kbps: 128,
      faststart: true,
      keep_metadata: true,
    },
    image: {
      format: "jpeg",
      quality: 85,
      max_edge: null,
      keep_metadata: true,
    },
    output: { mode: "subfolder", name: "_optimized" },
    name_suffix: null,
    on_conflict: "rename",
    skip_if_larger: true,
    skip_if_savings_below_pct: 5,
    concurrency: 2,
    preserve_timestamps: true,
  };
}

/// Presets shipped with the app. Their names are translated at render time.
export function builtInPresets(locale: Locale): BatchPreset[] {
  const base = defaultBatchSettings();
  return [
    {
      id: "builtin-max-savings",
      name: t(locale, "batch.settings.presetMaxSavings"),
      settings: {
        ...base,
        video: { ...base.video, codec: "h265", crf: 28, max_height: 1080 },
      },
    },
    {
      id: "builtin-balanced",
      name: t(locale, "batch.settings.presetBalanced"),
      settings: {
        ...base,
        video: { ...base.video, codec: "h264", crf: 23, max_height: null },
      },
    },
    {
      id: "builtin-remux",
      name: t(locale, "batch.settings.presetRemux"),
      settings: {
        ...base,
        video: { ...base.video, codec: "copy", audio: "copy" },
        skip_if_savings_below_pct: null,
      },
    },
    {
      id: "builtin-heic-jpeg",
      name: t(locale, "batch.settings.presetHeicJpeg"),
      settings: {
        ...base,
        image: { ...base.image, format: "jpeg", quality: 90, max_edge: null },
      },
    },
    {
      id: "builtin-web-images",
      name: t(locale, "batch.settings.presetWebImages"),
      settings: {
        ...base,
        image: { ...base.image, format: "webp", quality: 80, max_edge: 2560 },
      },
    },
  ];
}

/// Settings coming back from disk may name a destructive output mode; it is
/// downgraded so the confirmation dialog has to be answered again.
export function sanitizeStoredSettings(settings: BatchSettings): BatchSettings {
  if (settings.output.mode === "replace_original") {
    return { ...settings, output: { mode: "subfolder", name: "_optimized" } };
  }
  return settings;
}

/// Only the parts a preset actually defines: output folder, concurrency and
/// naming are the user's own choices and must not break the match.
function profileFingerprint(settings: BatchSettings): string {
  return stableStringify([
    settings.video,
    settings.image,
    settings.skip_if_savings_below_pct,
  ]);
}

function stableStringify(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  if (value && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).sort(
      ([a], [b]) => a.localeCompare(b),
    );
    return `{${entries.map(([k, v]) => `${k}:${stableStringify(v)}`).join(",")}}`;
  }
  return JSON.stringify(value) ?? "null";
}

/// Which preset the current settings correspond to, so the UI can say so
/// instead of leaving the user guessing after values change.
export function matchingPreset(
  presets: BatchPreset[],
  settings: BatchSettings,
): BatchPreset | null {
  const current = profileFingerprint(settings);
  return presets.find((preset) => profileFingerprint(preset.settings) === current) ?? null;
}

export function formatSize(bytes: number): string {
  if (bytes >= 1024 ** 3) return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
  if (bytes >= 1024 ** 2) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
}

export function savingsPercent(before: number, after: number): number {
  if (before <= 0) return 0;
  return Math.max(0, Math.round(100 - (after * 100) / before));
}

/// Every file the selection touches. Live Photos carry two paths and must be
/// converted together, otherwise the pair breaks.
export function selectedPaths(items: MediaItem[], selected: Set<string>): string[] {
  return items
    .filter((item) => selected.has(item.id))
    .flatMap((item) => item.paths);
}

export function dedupeItems(items: MediaItem[]): MediaItem[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    if (seen.has(item.id)) return false;
    seen.add(item.id);
    return true;
  });
}

export async function loadCapabilities(): Promise<FfmpegCapabilities> {
  return invokeLogged<FfmpegCapabilities>("get_ffmpeg_capabilities");
}

export async function loadQueueItems(): Promise<MediaItem[]> {
  return invokeLogged<MediaItem[]>("list_queue_items");
}

export async function pickFiles(): Promise<MediaItem[]> {
  const paths = await invokeLogged<string[]>("pick_media_files");
  if (paths.length === 0) return [];
  return invokeLogged<MediaItem[]>("describe_media_paths", { paths });
}

export async function scanFolder(
  path: string,
  recursive: boolean,
  excludeDirs: string[],
): Promise<MediaItem[]> {
  return invokeLogged<MediaItem[]>("scan_folder_media", {
    path,
    recursive,
    excludeDirs,
  });
}

export async function startJob(
  paths: string[],
  settings: BatchSettings,
): Promise<BatchJobStatus> {
  return invokeLogged<BatchJobStatus>("start_batch_job", { paths, settings });
}

export async function cancelJob(jobId: string): Promise<void> {
  await invokeLogged("cancel_batch_job", { jobId });
}

export async function activeJob(): Promise<BatchJobStatus | null> {
  return invokeLogged<BatchJobStatus | null>("get_active_batch_job");
}
