<script lang="ts">
  import { onMount } from "svelte";
  import BatchPanel from "$lib/components/BatchPanel.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import FolderPicker from "$lib/components/FolderPicker.svelte";
  import HelpOverlay from "$lib/components/HelpOverlay.svelte";
  import MetadataPanel from "$lib/components/MetadataPanel.svelte";
  import QuickOptimize from "$lib/components/QuickOptimize.svelte";
  import UpdateDialog from "$lib/components/UpdateDialog.svelte";
  import OptionsPanel from "$lib/components/OptionsPanel.svelte";
  import PhotoViewer from "$lib/components/PhotoViewer.svelte";
  import RenameInput from "$lib/components/RenameInput.svelte";
  import ShortcutBar from "$lib/components/ShortcutBar.svelte";
  import VideoTrimPanel from "$lib/components/VideoTrimPanel.svelte";
  import Toast from "$lib/components/Toast.svelte";
  import WelcomeScreen from "$lib/components/WelcomeScreen.svelte";
  import {
    getErrorLog,
    getErrorLogPath,
    initErrorReporting,
    invokeLogged,
    reportError,
  } from "$lib/errorReporter";
  import { defaultBatchSettings, matchingPreset, builtInPresets } from "$lib/batch";
  import {
    detectLocale,
    format,
    isLocale,
    t,
    translate,
    type Locale,
  } from "$lib/i18n";
  import {
    buildScreenshotBatchItems,
    buildScreenshotBatchJob,
    buildScreenshotVideoWorkspaceState,
    buildScreenshotWorkspaceState,
    getScreenshotMode,
    type ScreenshotMode,
  } from "$lib/screenshotDemo";
  import { modKey, modLabel, skipModLabel, isSkipShortcut } from "$lib/shortcuts";
  import {
    checkForUpdate,
    installUpdate,
    loadUpdateContext,
    type AvailableUpdate,
  } from "$lib/updater";
  import { formatBytes } from "$lib/utils";
  import type {
    ActionResult,
    AppSettings,
    BatchJobStatus,
    BatchSettings,
    FrontendState,
    LayoutMode,
    MediaItem,
    RenameMode,
    SortMode,
  } from "$lib/types";

  let locale = $state<Locale>("en");
  let showWelcome = $state(true);
  let dontShowAgain = $state(false);
  let renameValue = $state("");
  let showMetadata = $state(true);
  let videoWithSound = $state(false);
  let layoutMode = $state<LayoutMode>("sidebar");
  let showFolderPicker = $state(false);
  let showOptions = $state(false);
  let showBatch = $state(false);
  let batchJobRunning = $state(false);
  let batchInitialItems = $state<MediaItem[] | null>(null);
  let batchAutoStart = $state(false);
  let batchDemoJob = $state<BatchJobStatus | null>(null);
  let batchDemoStep = $state<"select" | "settings" | "run" | null>(null);
  let panelTab = $state<"rename" | "optimize">("rename");
  let trimNotice = $state("");
  let availableUpdate = $state<AvailableUpdate | null>(null);
  let releasesUrl = $state<string | null>(null);
  let repositoryUrl = $state<string | null>(null);
  let showUpdate = $state(false);
  let installingUpdate = $state(false);
  let updateProgress = $state(0);
  /// Settings for the quick picker in the Optimize tab, handed to the batch
  /// panel when the user runs it or opens the advanced view.
  let quickSettings = $state<BatchSettings>(defaultBatchSettings());
  const quickPresetId = $derived(
    matchingPreset(builtInPresets(locale), quickSettings)?.id ?? null,
  );
  let showHelp = $state(false);
  /// Pending yes/no question, rendered by `ConfirmDialog`.
  let confirmPrompt = $state<{ message: string; onConfirm: () => void } | null>(null);
  let folderQuery = $state("");
  let folderSelection = $state<string | null>(null);
  let toastMessage = $state("");
  let toastError = $state(false);
  let activeKey = $state("");
  let renameInput: HTMLInputElement | null = $state(null);
  let errorLogCount = $state(0);
  let errorLogPath = $state("logs/app-errors.jsonl");
  let skipUiPersist = $state(true);
  let actionInFlight = $state(false);
  let folderPickerInitialQuery = $state("");
  let videoRef = $state<HTMLVideoElement | null>(null);
  let ffmpegAvailable = $state(true);
  let pendingVideoTrim = $state(false);
  let showResumeBanner = $state(false);
  let trimPanel = $state<{
    setStartToPlayhead: () => void;
    setEndToPlayhead: () => void;
    getTrimRange: () => { start: number; end: number } | null;
    resetAfterApply: () => void;
  } | null>(null);

  const screenshotMode = $derived(getScreenshotMode());

  let appState = $state<FrontendState>({
    current_index: 0,
    total: 0,
    sort_mode: "exif_date",
    scan_recursive: false,
    rename_mode: "free",
    recent_folders: [],
    favorite_folders: [],
    existing_subfolders: [],
    stats: { renamed: 0, trashed: 0, moved: 0, skipped: 0 },
    session_complete: false,
  });

  const displayProgress = $derived.by(() => {
    const sessionTotal =
      appState.total + appState.stats.trashed + appState.stats.moved;
    const total = sessionTotal || appState.total;
    if (total === 0) {
      return { current: 0, total: 0, percent: 0 };
    }

    const actions =
      appState.stats.renamed +
      appState.stats.trashed +
      appState.stats.moved +
      appState.stats.skipped;
    const current = Math.min(actions + 1, total);

    return {
      current,
      total,
      percent: Math.min(100, (current / total) * 100),
    };
  });

  const hasWorkspace = $derived(
    !showWelcome &&
      !!appState.folder_path &&
      appState.total > 0 &&
      !appState.session_complete,
  );

  const workspaceDisabled = $derived(
    showWelcome || !appState.folder_path || !hasWorkspace || actionInFlight || batchJobRunning,
  );
  const chromeDisabled = $derived(showWelcome || actionInFlight);

  const showSessionComplete = $derived(
    !showWelcome &&
      !!appState.folder_path &&
      (appState.session_complete || appState.total === 0),
  );

  const sessionStatsLine = $derived(
    format(locale, "sessionStats", {
      renamed: appState.stats.renamed,
      trashed: appState.stats.trashed,
      moved: appState.stats.moved,
      skipped: appState.stats.skipped,
    }),
  );

  const sidebarLayout = $derived(layoutMode === "sidebar" && hasWorkspace);

  const batchQueueAvailable = $derived(!!appState.folder_path && appState.total > 0);

  $effect(() => {
    const _metadata = showMetadata;
    const _layout = layoutMode;
    const _videoSound = videoWithSound;
    if (skipUiPersist) return;
    void persistUiPreferences();
  });

  function applyScreenshotDemo(mode: ScreenshotMode) {
    locale = "en";
    skipUiPersist = true;

    if (mode === "welcome") {
      showWelcome = true;
      return;
    }

    showWelcome = false;
    showMetadata = true;
    layoutMode = "sidebar";

    if (mode.startsWith("batch-")) {
      appState = buildScreenshotWorkspaceState();
      batchInitialItems = buildScreenshotBatchItems();
      batchDemoJob =
        mode === "batch-progress" || mode === "batch-done"
          ? buildScreenshotBatchJob(mode === "batch-done")
          : null;
      batchDemoStep = mode === "batch-settings" ? "settings" : null;
      showBatch = true;
      return;
    }

    if (mode === "workspace-video") {
      renameValue = "Day at the park";
      appState = buildScreenshotVideoWorkspaceState();
      return;
    }

    renameValue = "Sunset at the beach";
    appState = buildScreenshotWorkspaceState();
  }

  async function refreshErrorLogMeta() {
    errorLogCount = (await getErrorLog()).length;
    errorLogPath = await getErrorLogPath();
  }

  onMount(() => {
    initErrorReporting();

    const demo = getScreenshotMode();
    if (demo) {
      applyScreenshotDemo(demo);
      window.addEventListener("keydown", handleKeydown);
      return () => window.removeEventListener("keydown", handleKeydown);
    }

    void (async () => {
      const settings = await invokeLogged<AppSettings>("get_app_settings");
      // A stored choice wins: detection is only for a first run. Reading it as
      // "es or detect" meant picking English on a Spanish machine never stuck.
      locale = isLocale(settings.locale) ? settings.locale : detectLocale();
      showWelcome = !settings.first_run_completed;
      showMetadata = settings.show_metadata ?? true;
      videoWithSound = settings.video_with_sound ?? false;
      layoutMode = settings.layout_mode ?? "sidebar";
      appState = await invokeLogged<FrontendState>("get_state");
      await refreshErrorLogMeta();
      ffmpegAvailable = await invokeLogged<boolean>("check_ffmpeg");

      if (settings.last_folder_path) {
        showToast(t(locale, "resumingFolder"));
        try {
          applyOpenFolderState(
            await invokeLogged<FrontendState>("open_folder", {
              path: settings.last_folder_path,
            }),
          );
          renameValue = "";
        } catch (error) {
          showToast(String(error), true, 8000);
        }
      }

      focusRenameInput();
      skipUiPersist = false;
      void scheduleUpdateCheck();
    })().catch((error) => {
      void reportError(String(error), { phase: "startup" });
      showToast(t(locale, "startupError"), true, 8000);
    });

    window.addEventListener("keydown", handleKeydown);
    return () => window.removeEventListener("keydown", handleKeydown);
  });

  async function scheduleUpdateCheck() {
    const context = await loadUpdateContext();
    releasesUrl = context?.releases_url ?? null;
    repositoryUrl = context?.repository_url ?? null;
    if (!context?.updater_configured) return;

    setTimeout(() => {
      void checkForUpdate().then((update) => {
        availableUpdate = update;
      });
    }, 4000);
  }

  async function runUpdateInstall() {
    installingUpdate = true;
    updateProgress = 0;
    try {
      await installUpdate((fraction) => (updateProgress = fraction));
    } catch (error) {
      installingUpdate = false;
      showToast(format(locale, "update.failed", { error: String(error) }), true, 10000);
    }
  }

  function focusRenameInput() {
    queueMicrotask(() => renameInput?.focus());
  }

  function flashKey(key: string) {
    activeKey = key;
    setTimeout(() => {
      if (activeKey === key) activeKey = "";
    }, 200);
  }

  function showToast(message: string, error = false, duration = 2200) {
    toastMessage = message;
    toastError = error;
    if (duration <= 0) return;
    setTimeout(() => {
      if (toastMessage === message) toastMessage = "";
    }, duration);
  }

  function applyOpenFolderState(state: FrontendState) {
    appState = state;
    showResumeBanner = false;
    if (state.session_reset) {
      showToast(t(locale, "sessionPositionReset"), false, 6000);
    } else if (state.resume_from && state.total > 0) {
      showResumeBanner = true;
    }

    if (state.subfolder_media_count && state.subfolder_media_count > 0) {
      showToast(
        format(locale, "subfolderMediaNotice", { count: state.subfolder_media_count }),
        false,
        8000,
      );
    }
  }

  function openBatchPanel() {
    batchInitialItems = null;
    batchAutoStart = false;
    showBatch = true;
  }

  /// "Advanced options" hands the current file and the quick settings over to
  /// the full panel.
  function openBatchForCurrentItem(autoStart = false) {
    if (!appState.item) return;
    batchInitialItems = [appState.item];
    batchAutoStart = autoStart;
    showBatch = true;
  }

  function dismissResumeBanner() {
    showResumeBanner = false;
  }

  function dismissToast() {
    toastMessage = "";
  }

  function isInteractiveTarget(target: HTMLElement | null): boolean {
    if (!target) return false;
    if (target.closest(".kbd-chip")) return true;
    const tag = target.tagName;
    return tag === "BUTTON" || tag === "SELECT" || tag === "VIDEO" || tag === "TEXTAREA";
  }

  async function runAction(action: () => Promise<void>) {
    if (actionInFlight || batchJobRunning) return;
    actionInFlight = true;
    try {
      await action();
    } finally {
      actionInFlight = false;
    }
  }

  function applyActionResult(result: ActionResult, options: { trimmed?: boolean } = {}) {
    appState = result.state;
    let message =
      result.success && options.trimmed
        ? t(locale, "trim.savedWithRename")
        : translate(locale, result.message_key, result.message_args);
    let duration = result.success ? 2200 : 8000;

    // Keyed off what the action was, not off the wording it produced.
    if (result.success && result.message_key === "action.trashed") {
      message = `${message} ${format(locale, "undoHint", { key: modLabel("Z") })}`;
      duration = 5000;
    }

    if (result.undo_history_trimmed) {
      message = `${message} ${t(locale, "action.undoHistoryTrimmed")}`;
    }

    showToast(message, !result.success, duration);
    if (result.success) {
      renameValue = "";
    }
    focusRenameInput();
    if (!result.success) {
      void reportError(result.message_key, {
        action: "command_result",
        args: result.message_args,
      });
    }
    void refreshErrorLogMeta();
  }

  async function openFolderDialog() {
    const folder = await invokeLogged<string | null>("pick_folder");
    if (!folder) return;
    try {
      applyOpenFolderState(await invokeLogged<FrontendState>("open_folder", { path: folder }));
      renameValue = "";
      focusRenameInput();
      await refreshErrorLogMeta();
    } catch (error) {
      showToast(String(error), true);
    }
  }

  async function applyPendingTrimIfAny(): Promise<boolean> {
    const range = trimPanel?.getTrimRange();
    if (!range) return true;

    const trimResult = await invokeLogged<ActionResult>("trim_current_video", {
      trimStart: range.start,
      trimEnd: range.end,
    });
    if (!trimResult.success) {
      applyActionResult(trimResult);
      return false;
    }

    appState = trimResult.state;
    if (videoRef) videoRef.load();
    trimPanel?.resetAfterApply();
    return true;
  }

  async function saveCurrent() {
    await runAction(async () => {
      const willTrim = !!trimPanel?.getTrimRange();
      const hasArmed = !!appState.armed_folder;
      const hasName = !!renameValue.trim();

      if (!hasArmed && !hasName) {
        showToast(t(locale, "writeName"), true, 5000);
        return;
      }

      if (!(await applyPendingTrimIfAny())) return;

      if (hasArmed && !hasName) {
        showToast(t(locale, "armedMoveEmpty"));
      }

      const result = await invokeLogged<ActionResult>("rename_current", {
        name: renameValue,
      });
      applyActionResult(result, { trimmed: willTrim });
    });
  }

  async function trashCurrent() {
    await runAction(async () => {
      const result = await invokeLogged<ActionResult>("trash_current");
      applyActionResult(result);
    });
  }

  async function applyVideoTrim(trimStart: number, trimEnd: number) {
    await runAction(async () => {
      const result = await invokeLogged<ActionResult>("trim_current_video", {
        trimStart,
        trimEnd,
      });
      applyActionResult(result);
      if (result.success) {
        if (videoRef) videoRef.load();
        trimPanel?.resetAfterApply();
        showTrimNotice(result.state.item?.size_bytes ?? 0);
      }
    });
  }

  function showTrimNotice(sizeBytes: number) {
    const message = format(locale, "trim.applied", {
      size: formatBytes(sizeBytes),
      key: modLabel("Z"),
    });
    trimNotice = message;
    setTimeout(() => {
      if (trimNotice === message) trimNotice = "";
    }, 12000);
  }

  async function skip(delta: number) {
    if (actionInFlight || batchJobRunning) return;
    trimNotice = "";
    try {
      appState = await invokeLogged<FrontendState>("skip_current", { delta });
      if (appState.session_complete) {
        renameValue = "";
        return;
      }
      renameValue = "";
      focusRenameInput();
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function restartQueue() {
    try {
      appState = await invokeLogged<FrontendState>("restart_queue");
      showResumeBanner = false;
      renameValue = "";
      if (appState.total === 0) {
        showToast(t(locale, "emptyQueue"));
      } else {
        focusRenameInput();
      }
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function dismissSessionComplete() {
    try {
      appState = await invokeLogged<FrontendState>("dismiss_session_complete");
      focusRenameInput();
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function undoLast() {
    await runAction(async () => {
      try {
        const result = await invokeLogged<ActionResult>("undo_last");
        applyActionResult(result);
      } catch (error) {
        showToast(String(error), true, 8000);
      }
    });
  }

  async function confirmFolder() {
    const folder = (folderQuery || folderSelection || "").trim();
    if (!folder) {
      showToast(t(locale, "chooseFolder"), true, 5000);
      return;
    }
    try {
      appState = await invokeLogged<FrontendState>("set_armed_folder", { folder });
      showFolderPicker = false;
      focusRenameInput();
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function toggleFavorite(folder: string) {
    try {
      appState = await invokeLogged<FrontendState>("toggle_favorite_folder", { folder });
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function applyOptions() {
    try {
      appState = await invokeLogged<FrontendState>("set_options", {
        sortMode: appState.sort_mode,
        scanRecursive: appState.scan_recursive,
        renameMode: appState.rename_mode,
      });
      await persistUiPreferences();
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function closeOptions() {
    showOptions = false;
    await applyOptions();
  }

  function closeHelp() {
    showHelp = false;
  }

  function closeFolderPicker() {
    const dirty = folderQuery.trim() !== folderPickerInitialQuery.trim();
    if (dirty && folderQuery.trim()) {
      // `window.confirm` would block the webview and ignore the app's
      // language; the answer comes back through `confirmPrompt` instead.
      confirmPrompt = {
        message: t(locale, "folderPicker.discard"),
        onConfirm: () => {
          confirmPrompt = null;
          showFolderPicker = false;
        },
      };
      return;
    }
    showFolderPicker = false;
  }

  async function persistUiPreferences() {
    try {
      await invokeLogged<AppSettings>("set_ui_preferences", {
        layoutMode,
        showMetadata,
        videoWithSound,
      });
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  async function toggleMetadata() {
    showMetadata = !showMetadata;
    flashKey(modLabel("M"));
  }

  async function changeLocale(next: Locale) {
    locale = next;
    await invokeLogged("set_locale", { locale: next });
  }

  async function finishWelcome() {
    showWelcome = false;
    if (dontShowAgain) {
      try {
        await invokeLogged("complete_first_run");
      } catch (error) {
        showToast(String(error), true, 8000);
      }
    }
    focusRenameInput();
  }

  function openFolderPicker() {
    folderQuery = appState.armed_folder ?? "";
    folderSelection = appState.armed_folder ?? null;
    folderPickerInitialQuery = folderQuery;
    showFolderPicker = true;
    flashKey(modLabel("F"));
  }

  async function disarmFolder() {
    try {
      appState = await invokeLogged<FrontendState>("set_armed_folder", { folder: null });
    } catch (error) {
      showToast(String(error), true, 8000);
    }
  }

  function handleModShortcut(key: string | null): boolean {
    if (!key) return false;

    if (key === "o") {
      showOptions = true;
      flashKey(modLabel("O"));
      return true;
    }

    if (key === "m") {
      void toggleMetadata();
      return true;
    }

    if (key === "b") {
      openBatchPanel();
      flashKey(modLabel("B"));
      return true;
    }

    if (!hasWorkspace || batchJobRunning) return false;

    if (key === "z") {
      flashKey("Undo");
      void undoLast();
      return true;
    }
    if (key === "f") {
      openFolderPicker();
      return true;
    }
    if (key === "d") {
      flashKey(modLabel("D"));
      void trashCurrent();
      return true;
    }

    return false;
  }

  function handleKeydown(event: KeyboardEvent) {
    // A pending question owns the keyboard until it is answered: both
    // listeners sit on `window`, so the dialog cannot stop this one.
    if (confirmPrompt) return;

    const target = event.target as HTMLElement | null;
    const inRenameInput =
      target?.id === "rename-input" ||
      (target?.tagName === "INPUT" && target?.classList.contains("rename-input"));

    if (showWelcome) {
      if (event.key === "?" || (event.shiftKey && event.key === "/")) {
        event.preventDefault();
        showHelp = true;
        return;
      }
      const mod = modKey(event);
      if (mod === "o") {
        event.preventDefault();
        showOptions = true;
        return;
      }
      if (mod === "b") {
        event.preventDefault();
        openBatchPanel();
        return;
      }
      return;
    }

    if (showFolderPicker) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeFolderPicker();
      }
      if (
        event.key === "Enter" &&
        !isInteractiveTarget(target) &&
        target?.id !== "folder-picker-input"
      ) {
        event.preventDefault();
        void confirmFolder();
      }
      return;
    }

    if (showBatch) {
      return;
    }

    if (showOptions) {
      if (event.key === "Escape") {
        event.preventDefault();
        void closeOptions();
      }
      return;
    }

    if (showHelp) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeHelp();
      }
      return;
    }

    const mod = modKey(event);
    if (mod && handleModShortcut(mod)) {
      event.preventDefault();
      return;
    }

    if (isSkipShortcut(event)) {
      event.preventDefault();
      if (!hasWorkspace || actionInFlight || batchJobRunning) return;
      flashKey(skipModLabel());
      void skip(1);
      return;
    }

    if (inRenameInput) {
      if (event.key === "Enter") {
        event.preventDefault();
        flashKey("Enter");
        void saveCurrent();
      }
      if (event.key === "Escape" && appState.armed_folder) {
        event.preventDefault();
        void disarmFolder();
      }
      return;
    }

    if (isInteractiveTarget(target)) return;

    if (!hasWorkspace) return;

    if (actionInFlight || batchJobRunning) return;

    if (appState.item?.is_video && trimPanel) {
      if (event.key === "[") {
        event.preventDefault();
        panelTab = "rename";
        trimPanel.setStartToPlayhead();
        return;
      }
      if (event.key === "]") {
        event.preventDefault();
        panelTab = "rename";
        trimPanel.setEndToPlayhead();
        return;
      }
    }

    if (event.key === "?" || (event.shiftKey && event.key === "/")) {
      event.preventDefault();
      showHelp = true;
      return;
    }

    if (event.key === "Escape" && appState.armed_folder) {
      event.preventDefault();
      void disarmFolder();
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      flashKey("Enter");
      void saveCurrent();
      return;
    }

    if (event.key === "ArrowRight") {
      event.preventDefault();
      flashKey("ArrowRight");
      void skip(1);
      return;
    }

    if (event.key === "ArrowLeft") {
      event.preventDefault();
      flashKey("ArrowLeft");
      void skip(-1);
      return;
    }
  }
</script>

<div
  class="app-shell"
  class:layout-sidebar={sidebarLayout}
  class:layout-bottom={layoutMode === "bottom" && hasWorkspace}
  class:screenshot-demo={!!screenshotMode && screenshotMode !== "welcome"}
  data-screenshot-ready={screenshotMode ?? undefined}
>
  <header class="app-header">
    <div class="brand">
      <div class="brand-mark">◈</div>
      <div>{t(locale, "appTitle")}</div>
    </div>

    <div class="progress-wrap">
      <div class="progress-bar">
        <div class="progress-fill" style={`width:${displayProgress.percent}%`}></div>
      </div>
      <div class="progress-label">
        {displayProgress.total === 0
          ? "0 / 0"
          : `${displayProgress.current} / ${displayProgress.total}`}
      </div>
    </div>

    <div class="toolbar-actions">
      {#if hasWorkspace && appState.current_index > 0}
        <button class="ghost-btn" onclick={() => void restartQueue()}>
          {t(locale, "restartQueue")}
        </button>
      {/if}
      <div class="locale-switch">
        <button class:active={locale === "en"} onclick={() => changeLocale("en")}>EN</button>
        <button class:active={locale === "es"} onclick={() => changeLocale("es")}>ES</button>
      </div>
      {#if availableUpdate}
        <button
          class="ghost-btn update-chip"
          onclick={() => (showUpdate = true)}
          title={format(locale, "update.versions", {
            current: availableUpdate.currentVersion,
            next: availableUpdate.version,
          })}
        >
          <span class="update-dot" aria-hidden="true"></span>
          {t(locale, "update.available")}
        </button>
      {/if}
      {#if !showWelcome}
        <button class="ghost-btn" onclick={openBatchPanel}>
          {t(locale, "batch.open")}
        </button>
        <button class="ghost-btn" onclick={() => (showOptions = true)} disabled={chromeDisabled}>
          {t(locale, "shortcuts.options")}
        </button>
      {/if}
      <button class="primary-btn" onclick={openFolderDialog}>{t(locale, "openFolder")}</button>
    </div>
  </header>

  {#if appState.armed_folder && hasWorkspace}
    <div class="armed-banner" title={appState.armed_folder}>
      {format(locale, "armedBanner", { folder: appState.armed_folder })}
    </div>
  {/if}

  {#if showResumeBanner && hasWorkspace}
    <div class="resume-banner">
      <span>
        {format(locale, "sessionResumeBanner", {
          current: displayProgress.current,
          total: displayProgress.total,
        })}
      </span>
      <div class="resume-banner-actions">
        <button class="primary-btn" onclick={() => void restartQueue()}>
          {t(locale, "restartQueue")}
        </button>
        <button class="ghost-btn" onclick={dismissResumeBanner}>
          {t(locale, "sessionResumeContinue")}
        </button>
      </div>
    </div>
  {/if}

  {#if showWelcome}
    <WelcomeScreen {locale} bind:dontShowAgain onStart={finishWelcome} hideSupport={!!screenshotMode} />
  {:else if showSessionComplete}
    <div class="empty-state">
      <div class="welcome-card session-complete-card">
        <h2>
          {appState.total === 0
            ? t(locale, "emptyQueue")
            : t(locale, "sessionReachedEnd")}
        </h2>
        <p class="session-stats-line">{sessionStatsLine}</p>
        <div class="modal-actions">
          <button class="primary-btn" onclick={() => void restartQueue()}>
            {t(locale, "restartQueue")}
          </button>
          {#if appState.total > 0}
            <button class="ghost-btn" onclick={() => void dismissSessionComplete()}>
              {t(locale, "continueReviewing")}
            </button>
          {/if}
          <button class="ghost-btn" onclick={openFolderDialog}>
            {t(locale, "openFolder")}
          </button>
          <button class="ghost-btn" onclick={openBatchPanel}>
            {t(locale, "batch.open")}
          </button>
        </div>
      </div>
    </div>
  {:else if !appState.folder_path}
    <div class="empty-state">
      <div class="welcome-card">
        <h2>{t(locale, "noFolder")}</h2>
        <div class="modal-actions">
          <button class="primary-btn" onclick={openFolderDialog}>{t(locale, "openFolder")}</button>
          <button class="ghost-btn" onclick={openBatchPanel}>
            {t(locale, "batch.open")}
          </button>
          <button class="ghost-btn" onclick={() => (showOptions = true)} disabled={chromeDisabled}>
            {t(locale, "shortcuts.options")}
          </button>
        </div>
      </div>
    </div>
  {:else}
    <section class="workspace">
      <div class="preview-column">
        <PhotoViewer
          {locale}
          item={appState.item}
          bind:videoRef
          demoMode={!!screenshotMode}
          {videoWithSound}
          onError={(message) => showToast(message, true, 8000)}
        />
      </div>
      <aside class="side-panel">
        <div class="control-panel">
          <div class="panel-tabs" role="tablist">
            <button
              type="button"
              role="tab"
              aria-selected={panelTab === "rename"}
              class:active={panelTab === "rename"}
              onclick={() => (panelTab = "rename")}
            >
              {t(locale, "sidePanel.tabRename")}
              {#if pendingVideoTrim}
                <span class="tab-dot" aria-hidden="true"></span>
              {/if}
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={panelTab === "optimize"}
              class:active={panelTab === "optimize"}
              onclick={() => (panelTab = "optimize")}
            >
              {t(locale, "sidePanel.tabOptimize")}
            </button>
          </div>

          <div class="panel-tab-body" class:hidden={panelTab !== "rename"}>
            <RenameInput
              {locale}
              bind:value={renameValue}
              bind:inputRef={renameInput}
              armedFolder={appState.armed_folder}
              compact={sidebarLayout}
              pendingTrim={pendingVideoTrim}
            />
            <MetadataPanel {locale} item={appState.item} bind:visible={showMetadata} />
            {#if appState.item?.is_video}
              <VideoTrimPanel
                bind:this={trimPanel}
                {locale}
                bind:videoRef
                bind:pendingTrim={pendingVideoTrim}
                {ffmpegAvailable}
                disabled={workspaceDisabled}
                screenshotDemo={screenshotMode === "workspace-video"}
                notice={trimNotice}
                onApply={(start, end) => void applyVideoTrim(start, end)}
              />
            {/if}
          </div>

          <div class="panel-tab-body" class:hidden={panelTab !== "optimize"}>
            <QuickOptimize
              {locale}
              item={appState.item}
              bind:settings={quickSettings}
              activePresetId={quickPresetId}
              disabled={workspaceDisabled}
              onOptimize={() => openBatchForCurrentItem(true)}
              onAdvanced={() => openBatchForCurrentItem(false)}
            />
          </div>

        </div>
        {#if sidebarLayout}
          {@render shortcutBar(true)}
        {/if}
      </aside>
    </section>
  {/if}

  {#if !sidebarLayout}
    {@render shortcutBar(false)}
  {/if}
</div>

{#snippet shortcutBar(vertical: boolean)}
  <ShortcutBar
    {locale}
    progressCurrent={displayProgress.current}
    progressTotal={displayProgress.total}
    {activeKey}
    {vertical}
    pendingTrim={pendingVideoTrim}
    disabled={workspaceDisabled}
    chromeDisabled={chromeDisabled}
    onSave={() => {
      flashKey("Enter");
      void saveCurrent();
    }}
    onFolder={openFolderPicker}
    onDelete={() => {
      flashKey(modLabel("D"));
      void trashCurrent();
    }}
    onSkip={() => {
      flashKey(skipModLabel());
      void skip(1);
    }}
    onPrev={() => {
      flashKey("ArrowLeft");
      void skip(-1);
    }}
    onNext={() => {
      flashKey("ArrowRight");
      void skip(1);
    }}
    onUndo={() => {
      flashKey("Undo");
      void undoLast();
    }}
    onInfo={() => {
      void toggleMetadata();
    }}
    onOptions={() => (showOptions = true)}
    onHelp={() => (showHelp = true)}
  />
{/snippet}

<FolderPicker
  {locale}
  open={showFolderPicker}
  bind:query={folderQuery}
  bind:selected={folderSelection}
  favorites={appState.favorite_folders}
  recent={appState.recent_folders}
  existing={appState.existing_subfolders}
  onConfirm={confirmFolder}
  onClose={closeFolderPicker}
  onToggleFavorite={toggleFavorite}
/>

<OptionsPanel
  {locale}
  open={showOptions}
  bind:sortMode={appState.sort_mode}
  bind:scanRecursive={appState.scan_recursive}
  bind:renameMode={appState.rename_mode}
  bind:layoutMode
  bind:videoWithSound
  {repositoryUrl}
  errorLogCount={errorLogCount}
  errorLogPath={errorLogPath}
  onClose={closeOptions}
  onLocaleChange={changeLocale}
/>

<BatchPanel
  {locale}
  open={showBatch}
  hasQueue={batchQueueAvailable}
  initialItems={batchInitialItems}
  initialSettings={batchInitialItems && !screenshotMode ? quickSettings : null}
  autoStart={batchAutoStart}
  demoJob={batchDemoJob}
  demoStep={batchDemoStep}
  demoMode={!!screenshotMode}
  onRunningChange={(running) => (batchJobRunning = running)}
  onClose={() => {
    showBatch = false;
    batchInitialItems = null;
    batchAutoStart = false;
    focusRenameInput();
  }}
  onSessionChanged={(state) => {
    appState = state;
  }}
  onError={(message) => showToast(message, true, 8000)}
/>

<UpdateDialog
  {locale}
  open={showUpdate}
  update={availableUpdate}
  {releasesUrl}
  installing={installingUpdate}
  progress={updateProgress}
  onInstall={() => void runUpdateInstall()}
  onClose={() => (showUpdate = false)}
/>

<HelpOverlay {locale} open={showHelp} {repositoryUrl} onClose={closeHelp} />

<ConfirmDialog
  {locale}
  open={!!confirmPrompt}
  message={confirmPrompt?.message ?? ""}
  onConfirm={() => confirmPrompt?.onConfirm()}
  onCancel={() => (confirmPrompt = null)}
/>

<Toast message={toastMessage} error={toastError} onDismiss={dismissToast} />
