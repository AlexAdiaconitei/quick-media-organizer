import { invokeLogged } from "./errorReporter";
import { isTauriAvailable } from "./batch";

export interface UpdateContext {
  current_version: string;
  updater_configured: boolean;
  releases_url?: string | null;
}

export interface AvailableUpdate {
  version: string;
  currentVersion: string;
  notes: string;
  date?: string | null;
}

/// Kept out of the component so the plugin is only imported when it is usable:
/// in a plain browser there is no IPC and the import would throw.
type UpdateHandle = {
  version: string;
  currentVersion: string;
  body?: string;
  date?: string;
  downloadAndInstall: (
    onEvent?: (event: { event: string; data?: { contentLength?: number; chunkLength?: number } }) => void,
  ) => Promise<void>;
};

let pending: UpdateHandle | null = null;

export async function loadUpdateContext(): Promise<UpdateContext | null> {
  if (!isTauriAvailable()) return null;
  try {
    return await invokeLogged<UpdateContext>("get_update_context");
  } catch {
    return null;
  }
}

/// Returns the update when one is available, or null. Never throws: a machine
/// that is offline, or a build without an update endpoint, is not an error the
/// user needs to see.
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  if (!isTauriAvailable()) return null;
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = (await check()) as UpdateHandle | null;
    if (!update) {
      pending = null;
      return null;
    }
    pending = update;
    return {
      version: update.version,
      currentVersion: update.currentVersion,
      notes: (update.body ?? "").trim(),
      date: update.date ?? null,
    };
  } catch {
    pending = null;
    return null;
  }
}

/// Downloads and installs the update, reporting 0..1 progress, then restarts.
export async function installUpdate(onProgress: (fraction: number) => void): Promise<void> {
  if (!pending) throw new Error("No update is pending.");

  let total = 0;
  let received = 0;

  await pending.downloadAndInstall((event) => {
    if (event.event === "Started") {
      total = event.data?.contentLength ?? 0;
      onProgress(0);
    } else if (event.event === "Progress") {
      received += event.data?.chunkLength ?? 0;
      onProgress(total > 0 ? Math.min(1, received / total) : 0);
    } else if (event.event === "Finished") {
      onProgress(1);
    }
  });

  const { relaunch } = await import("@tauri-apps/plugin-process");
  await relaunch();
}
