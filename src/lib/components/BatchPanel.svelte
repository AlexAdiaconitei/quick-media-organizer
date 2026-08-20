<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import BatchProgress from "./BatchProgress.svelte";
  import BatchReplaceConfirmDialog from "./BatchReplaceConfirmDialog.svelte";
  import BatchSelectGrid from "./BatchSelectGrid.svelte";
  import BatchSettingsForm from "./BatchSettingsForm.svelte";
  import {
    activeJob,
    builtInPresets,
    cancelJob,
    dedupeItems,
    defaultBatchSettings,
    formatSize,
    isTauriAvailable,
    loadCapabilities,
    loadQueueItems,
    pickFiles,
    sanitizeStoredSettings,
    scanFolder,
    selectedPaths,
    startJob,
  } from "../batch";
  import { invokeLogged } from "../errorReporter";
  import { format, t, type Locale } from "../i18n";
  import type {
    BatchItemStatus,
    BatchJobStatus,
    BatchPreset,
    BatchProgressSummary,
    BatchSettings,
    FfmpegCapabilities,
    FrontendState,
    MediaItem,
  } from "../types";

  let {
    locale,
    open,
    hasQueue,
    onClose,
    onSessionChanged,
    onError,
  }: {
    locale: Locale;
    open: boolean;
    hasQueue: boolean;
    onClose: () => void;
    onSessionChanged: (state: FrontendState) => void;
    onError: (message: string) => void;
  } = $props();

  type Step = "select" | "settings" | "run";

  let step = $state<Step>("select");
  let items = $state<MediaItem[]>([]);
  let selected = $state<Set<string>>(new Set());
  let settings = $state<BatchSettings>(defaultBatchSettings());
  let capabilities = $state<FfmpegCapabilities>({
    available: true,
    h264: true,
    h265: true,
    av1: false,
    webp: true,
    avif: false,
    heic_decode: true,
    version: null,
  });
  let savedPresets = $state<BatchPreset[]>([]);
  let job = $state<BatchJobStatus | null>(null);
  let cancelling = $state(false);
  let busy = $state(false);
  let recursiveScan = $state(true);
  let showReplaceConfirm = $state(false);
  let loaded = $state(false);

  const presets = $derived([...builtInPresets(locale), ...savedPresets]);
  const selectedItems = $derived(items.filter((item) => selected.has(item.id)));
  const selectedBytes = $derived(
    selectedItems.reduce((sum, item) => sum + item.size_bytes, 0),
  );
  const hasVideos = $derived(selectedItems.some((item) => item.is_video));
  const hasImages = $derived(selectedItems.some((item) => !item.is_video));
  const hasHeic = $derived(
    selectedItems.some((item) => ["heic", "heif"].includes(item.extension)),
  );
  const replacesOriginals = $derived(settings.output.mode === "replace_original");
  const jobRunning = $derived(!!job?.running);
  const backupHint = $derived(
    `${selectedItems[0]?.paths[0]?.replace(/[/\\][^/\\]*$/, "") ?? ""}/.quick-media-organizer/batch-backups/`,
  );

  /// Never throws when the IPC bridge is missing, so running the frontend in a
  /// plain browser degrades instead of spamming unhandled rejections.
  function safeListen<T>(
    event: string,
    handler: (payload: T) => void,
  ): Promise<UnlistenFn | null> {
    try {
      return listen<T>(event, (message) => handler(message.payload)).catch(() => null);
    } catch {
      return Promise.resolve(null);
    }
  }

  onMount(() => {
    if (!isTauriAvailable()) return;

    const unlisteners = [
      safeListen<BatchItemStatus>("batch://item", applyItemUpdate),
      safeListen<BatchProgressSummary>("batch://progress", applyProgress),
      safeListen<BatchJobStatus>("batch://done", (job) => void applyDone(job)),
    ];

    return () => {
      for (const pending of unlisteners) {
        void pending.then((unlisten) => unlisten?.());
      }
    };
  });

  $effect(() => {
    if (!open || loaded) return;
    loaded = true;
    void initialize();
  });

  async function initialize() {
    if (!isTauriAvailable()) {
      // Browser preview: show the panel but make it clear nothing can run.
      capabilities = { ...capabilities, available: false };
      return;
    }

    busy = true;
    try {
      capabilities = await loadCapabilities();
      savedPresets = await invokeLogged<BatchPreset[]>("get_batch_presets");
      const stored = await invokeLogged<BatchSettings | null>("get_last_batch_settings");
      if (stored) settings = sanitizeStoredSettings(stored);

      const running = await activeJob();
      if (running) {
        job = running;
        step = "run";
      }

      if (hasQueue && items.length === 0) {
        items = await loadQueueItems();
        selected = new Set(items.map((item) => item.id));
      }
    } catch (error) {
      onError(String(error));
    } finally {
      busy = false;
    }
  }

  function applyItemUpdate(update: BatchItemStatus) {
    if (!job) return;
    const index = job.items.findIndex((item) => item.id === update.id);
    if (index < 0) return;
    job.items[index] = update;
  }

  function applyProgress(summary: BatchProgressSummary) {
    if (!job || job.job_id !== summary.job_id) return;
    job.done = summary.done;
    job.failed = summary.failed;
    job.skipped = summary.skipped;
    job.bytes_before = summary.bytes_before;
    job.bytes_after = summary.bytes_after;
  }

  async function applyDone(finishedJob: BatchJobStatus) {
    job = finishedJob;
    cancelling = false;
    try {
      // Registers the undo entry for replaced originals and rebuilds the queue.
      const state = await invokeLogged<FrontendState>("finalize_batch_job", {
        jobId: finishedJob.job_id,
      });
      onSessionChanged(state);
    } catch (error) {
      onError(String(error));
    }
  }

  async function addFiles() {
    busy = true;
    try {
      const added = await pickFiles();
      mergeItems(added);
    } catch (error) {
      onError(String(error));
    } finally {
      busy = false;
    }
  }

  async function addFolder() {
    busy = true;
    try {
      const folder = await invokeLogged<string | null>("pick_folder");
      if (!folder) return;
      const exclude =
        settings.output.mode === "custom_folder" && settings.output.path
          ? [settings.output.path]
          : [];
      const added = await scanFolder(folder, recursiveScan, exclude);
      if (added.length === 0) {
        onError(t(locale, "batch.select.folderEmpty"));
        return;
      }
      mergeItems(added);
    } catch (error) {
      onError(String(error));
    } finally {
      busy = false;
    }
  }

  function mergeItems(added: MediaItem[]) {
    const merged = dedupeItems([...items, ...added]);
    const addedIds = new Set(added.map((item) => item.id));
    items = merged;
    selected = new Set([...selected, ...addedIds]);
  }

  async function pickOutputFolder() {
    try {
      const folder = await invokeLogged<string | null>("pick_folder");
      if (!folder) return;
      settings.output = { mode: "custom_folder", path: folder };
    } catch (error) {
      onError(String(error));
    }
  }

  function applyPreset(preset: BatchPreset) {
    // Output mode is part of the preset, but a destructive one never survives
    // a load: it has to go through the confirmation dialog again.
    settings = sanitizeStoredSettings(structuredClone(preset.settings));
  }

  async function savePreset(name: string) {
    try {
      savedPresets = await invokeLogged<BatchPreset[]>("save_batch_preset", {
        preset: {
          id: `user-${Date.now()}`,
          name,
          settings: sanitizeStoredSettings(structuredClone($state.snapshot(settings))),
        },
      });
    } catch (error) {
      onError(String(error));
    }
  }

  async function deletePreset(id: string) {
    try {
      savedPresets = await invokeLogged<BatchPreset[]>("delete_batch_preset", { id });
    } catch (error) {
      onError(String(error));
    }
  }

  function confirmReplaceMode() {
    settings.output = { mode: "replace_original", backup: true, confirmed: true };
    showReplaceConfirm = false;
  }

  function cancelReplaceMode() {
    showReplaceConfirm = false;
    if (settings.output.mode === "replace_original") {
      settings.output = { mode: "subfolder", name: "_optimized" };
    }
  }

  async function start() {
    const paths = selectedPaths(items, selected);
    if (paths.length === 0) {
      onError(t(locale, "batch.run.noSelection"));
      return;
    }
    if (!capabilities.available) {
      onError(t(locale, "batch.ffmpegMissing"));
      return;
    }

    busy = true;
    try {
      job = await startJob(paths, $state.snapshot(settings));
      step = "run";
    } catch (error) {
      onError(String(error));
    } finally {
      busy = false;
    }
  }

  async function cancel() {
    if (!job) return;
    cancelling = true;
    try {
      await cancelJob(job.job_id);
    } catch (error) {
      cancelling = false;
      onError(String(error));
    }
  }

  function close() {
    onClose();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !showReplaceConfirm) {
      event.preventDefault();
      close();
    }
  }
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
  <div class="modal-backdrop batch-backdrop" role="presentation" onclick={close}>
    <div
      class="modal-card batch-card"
      role="dialog"
      aria-modal="true"
      aria-labelledby="batch-title"
      tabindex="-1"
      onclick={(event) => event.stopPropagation()}
    >
      <header class="batch-header">
        <div>
          <h2 id="batch-title">{t(locale, "batch.title")}</h2>
          <p class="batch-subtitle">{t(locale, "batch.subtitle")}</p>
        </div>
        <div class="batch-steps">
          <span class:active={step === "select"}>{t(locale, "batch.stepSelect")}</span>
          <span class:active={step === "settings"}>{t(locale, "batch.stepSettings")}</span>
          <span class:active={step === "run"}>{t(locale, "batch.stepRun")}</span>
        </div>
      </header>

      {#if !capabilities.available}
        <p class="batch-warning">{t(locale, "batch.ffmpegMissing")}</p>
      {:else if hasHeic && !capabilities.heic_decode}
        <p class="batch-warning">{t(locale, "batch.heicWarning")}</p>
      {/if}

      <div class="batch-body">
        {#if step === "select"}
          <div class="batch-sources">
            <button type="button" class="ghost-btn" disabled={busy} onclick={addFolder}>
              {t(locale, "batch.select.addFolder")}
            </button>
            <button type="button" class="ghost-btn" disabled={busy} onclick={addFiles}>
              {t(locale, "batch.select.addFiles")}
            </button>
            <label class="checkbox-row inline">
              <input type="checkbox" bind:checked={recursiveScan} />
              <span>{t(locale, "batch.select.includeSubfolders")}</span>
            </label>
          </div>
          <BatchSelectGrid {locale} {items} bind:selected {busy} />
        {:else if step === "settings"}
          <BatchSettingsForm
            {locale}
            bind:settings
            {capabilities}
            {hasVideos}
            {hasImages}
            {presets}
            onSavePreset={(name) => void savePreset(name)}
            onDeletePreset={(id) => void deletePreset(id)}
            onApplyPreset={applyPreset}
            onPickOutputFolder={() => void pickOutputFolder()}
            onRequestReplaceMode={() => (showReplaceConfirm = true)}
          />
        {:else if job}
          <BatchProgress {locale} {job} {cancelling} onCancel={() => void cancel()} />
        {/if}
      </div>

      <div class="modal-actions batch-actions">
        {#if step === "settings"}
          <button type="button" class="ghost-btn" onclick={() => (step = "select")}>
            {t(locale, "batch.back")}
          </button>
        {/if}

        {#if step === "select"}
          <button
            type="button"
            class="primary-btn"
            disabled={selected.size === 0 || busy}
            onclick={() => (step = "settings")}
          >
            {t(locale, "batch.next")}
          </button>
        {:else if step === "settings"}
          <button
            type="button"
            class={replacesOriginals ? "danger-btn" : "primary-btn"}
            disabled={busy || selected.size === 0 || !capabilities.available}
            onclick={() => void start()}
          >
            {format(locale, replacesOriginals ? "batch.run.startReplace" : "batch.run.start", {
              count: selectedItems.length,
            })}
          </button>
        {:else if !jobRunning}
          <button
            type="button"
            class="ghost-btn"
            onclick={() => {
              job = null;
              step = "select";
            }}
          >
            {t(locale, "batch.back")}
          </button>
        {/if}

        <button type="button" class="ghost-btn" onclick={close}>
          {t(locale, "batch.close")}
        </button>
      </div>

      {#if step === "settings" && selected.size > 0}
        <p class="batch-footnote">
          {format(locale, "batch.select.selected", {
            count: selectedItems.length,
            size: formatSize(selectedBytes),
          })}
        </p>
      {/if}
    </div>
  </div>
{/if}

<BatchReplaceConfirmDialog
  {locale}
  open={showReplaceConfirm}
  fileCount={selectedItems.length}
  totalBytes={selectedBytes}
  backupPath={backupHint}
  onConfirm={confirmReplaceMode}
  onCancel={cancelReplaceMode}
/>
