<script lang="ts">
  import Select from "./Select.svelte";
  import Switch from "./Switch.svelte";
  import { format, t, type Locale } from "../i18n";
  import type {
    AudioMode,
    BatchPreset,
    BatchSettings,
    ConflictPolicy,
    FfmpegCapabilities,
    ImageFormat,
    VideoCodec,
  } from "../types";

  let {
    locale,
    settings = $bindable<BatchSettings>(),
    capabilities,
    hasVideos,
    hasImages,
    presets,
    activePresetId,
    onSavePreset,
    onDeletePreset,
    onApplyPreset,
    onPickOutputFolder,
    onRequestReplaceMode,
  }: {
    locale: Locale;
    settings: BatchSettings;
    capabilities: FfmpegCapabilities;
    hasVideos: boolean;
    hasImages: boolean;
    presets: BatchPreset[];
    /// Preset the current values correspond to, or null when hand-tuned.
    activePresetId: string | null;
    onSavePreset: (name: string) => void;
    onDeletePreset: (id: string) => void;
    onApplyPreset: (preset: BatchPreset) => void;
    onPickOutputFolder: () => void;
    onRequestReplaceMode: () => void;
  } = $props();

  // Follows the selection until the user picks a tab explicitly.
  let tabOverride = $state<"video" | "image" | null>(null);
  const tab = $derived(tabOverride ?? (hasVideos ? "video" : "image"));
  let presetName = $state("");

  const outputMode = $derived(settings.output.mode);
  const customFolder = $derived(
    settings.output.mode === "custom_folder" ? settings.output.path : "",
  );
  const subfolderName = $derived(
    settings.output.mode === "subfolder" ? settings.output.name : "_optimized",
  );

  const codecOptions = $derived(
    [
      capabilities.h265 && {
        value: "h265" as VideoCodec,
        label: t(locale, "batch.settings.codecH265"),
      },
      capabilities.h264 && {
        value: "h264" as VideoCodec,
        label: t(locale, "batch.settings.codecH264"),
      },
      capabilities.av1 && {
        value: "av1" as VideoCodec,
        label: t(locale, "batch.settings.codecAv1"),
      },
      { value: "copy" as VideoCodec, label: t(locale, "batch.settings.codecCopy") },
    ].filter(Boolean) as { value: VideoCodec; label: string }[],
  );

  const speedOptions = $derived([
    { value: "slow", label: t(locale, "batch.settings.speedSlow") },
    { value: "medium", label: t(locale, "batch.settings.speedMedium") },
    { value: "fast", label: t(locale, "batch.settings.speedFast") },
  ]);

  // 0 stands for "keep what the source has", so the list can stay numeric.
  const heightOptions = $derived([
    { value: 0, label: t(locale, "batch.settings.keepResolution") },
    { value: 720, label: "720p" },
    { value: 1080, label: "1080p" },
    { value: 1440, label: "1440p" },
    { value: 2160, label: "2160p (4K)" },
  ]);

  const fpsOptions = $derived([
    { value: 0, label: t(locale, "batch.settings.keepFps") },
    { value: 24, label: "24" },
    { value: 30, label: "30" },
    { value: 60, label: "60" },
  ]);

  const audioOptions = $derived([
    { value: "aac" as AudioMode, label: t(locale, "batch.settings.audioAac") },
    { value: "copy" as AudioMode, label: t(locale, "batch.settings.audioCopy") },
    { value: "drop" as AudioMode, label: t(locale, "batch.settings.audioDrop") },
  ]);

  const formatOptions = $derived(
    [
      { value: "jpeg" as ImageFormat, label: t(locale, "batch.settings.formatJpeg") },
      capabilities.webp && {
        value: "webp" as ImageFormat,
        label: t(locale, "batch.settings.formatWebp"),
      },
      capabilities.avif && {
        value: "avif" as ImageFormat,
        label: t(locale, "batch.settings.formatAvif"),
      },
      { value: "png" as ImageFormat, label: t(locale, "batch.settings.formatPng") },
      { value: "keep" as ImageFormat, label: t(locale, "batch.settings.formatKeep") },
    ].filter(Boolean) as { value: ImageFormat; label: string }[],
  );

  const edgeOptions = $derived([
    { value: 0, label: t(locale, "batch.settings.keepSize") },
    { value: 1920, label: "1920 px" },
    { value: 2560, label: "2560 px" },
    { value: 3840, label: "3840 px" },
  ]);

  const conflictOptions = $derived([
    { value: "rename" as ConflictPolicy, label: t(locale, "batch.settings.conflictRename") },
    { value: "skip" as ConflictPolicy, label: t(locale, "batch.settings.conflictSkip") },
    { value: "overwrite" as ConflictPolicy, label: t(locale, "batch.settings.conflictOverwrite") },
  ]);

  function chooseOutput(mode: "subfolder" | "custom_folder" | "replace_original") {
    if (mode === "replace_original") {
      // Never flipped here: the parent opens the confirmation dialog and only
      // sets the mode if the user goes through with it.
      onRequestReplaceMode();
      return;
    }
    if (mode === "subfolder") {
      settings.output = { mode: "subfolder", name: subfolderName || "_optimized" };
      return;
    }
    settings.output = { mode: "custom_folder", path: customFolder };
    if (!customFolder) onPickOutputFolder();
  }

  function setSubfolderName(value: string) {
    settings.output = { mode: "subfolder", name: value };
  }
</script>

<div class="batch-settings">
  {#if presets.length > 0}
    <div class="batch-presets">
      <div class="batch-preset-head">
        <span class="field-label">{t(locale, "batch.settings.presets")}</span>
        <span class="batch-active-preset">
          {#if activePresetId}
            {presets.find((preset) => preset.id === activePresetId)?.name}
          {:else}
            {t(locale, "batch.settings.presetCustom")}
          {/if}
        </span>
      </div>
      <div class="batch-preset-list">
        {#each presets as preset (preset.id)}
          <span class="batch-preset-chip" class:active={preset.id === activePresetId}>
            <button
              type="button"
              class="ghost-btn"
              aria-pressed={preset.id === activePresetId}
              onclick={() => onApplyPreset(preset)}
            >
              {preset.name}
            </button>
            {#if !preset.id.startsWith("builtin-")}
              <button
                type="button"
                class="link-btn"
                title={t(locale, "batch.settings.presetDelete")}
                onclick={() => onDeletePreset(preset.id)}
              >
                ×
              </button>
            {/if}
          </span>
        {/each}
      </div>
      <div class="batch-preset-save">
        <input
          type="text"
          bind:value={presetName}
          placeholder={t(locale, "batch.settings.presetPlaceholder")}
        />
        <button
          type="button"
          class="ghost-btn"
          disabled={!presetName.trim()}
          onclick={() => {
            onSavePreset(presetName.trim());
            presetName = "";
          }}
        >
          {t(locale, "batch.settings.presetSave")}
        </button>
      </div>
    </div>
  {/if}

  {#if hasVideos && hasImages}
    <div class="batch-tabs">
      <button type="button" class:active={tab === "video"} onclick={() => (tabOverride = "video")}>
        {t(locale, "batch.settings.videoTab")}
      </button>
      <button type="button" class:active={tab === "image"} onclick={() => (tabOverride = "image")}>
        {t(locale, "batch.settings.imageTab")}
      </button>
    </div>
  {/if}

  {#if hasVideos && (tab === "video" || !hasImages)}
    <div class="options-grid">
      <div class="field-label">
        {t(locale, "batch.settings.codec")}
        <Select
          value={settings.video.codec}
          options={codecOptions}
          onchange={(codec) => (settings.video.codec = codec)}
          ariaLabel={t(locale, "batch.settings.codec")}
        />
      </div>

      {#if settings.video.codec !== "copy"}
        <label class="field-label">
          {format(locale, "batch.settings.crf", { value: settings.video.crf })}
          <input type="range" min="16" max="35" bind:value={settings.video.crf} />
          <small class="option-hint">{t(locale, "batch.settings.crfHint")}</small>
        </label>

        <div class="field-label narrow">
          {t(locale, "batch.settings.speed")}
          <Select
            value={settings.video.speed_preset}
            options={speedOptions}
            onchange={(speed) => (settings.video.speed_preset = speed)}
            ariaLabel={t(locale, "batch.settings.speed")}
          />
        </div>

        <div class="field-label narrow">
          {t(locale, "batch.settings.maxHeight")}
          <Select
            value={settings.video.max_height ?? 0}
            options={heightOptions}
            onchange={(height) => (settings.video.max_height = height || null)}
            ariaLabel={t(locale, "batch.settings.maxHeight")}
          />
        </div>

        <div class="field-label narrow">
          {t(locale, "batch.settings.maxFps")}
          <Select
            value={settings.video.max_fps ?? 0}
            options={fpsOptions}
            onchange={(fps) => (settings.video.max_fps = fps || null)}
            ariaLabel={t(locale, "batch.settings.maxFps")}
          />
        </div>

        <div class="field-label narrow">
          {t(locale, "batch.settings.audio")}
          <Select
            value={settings.video.audio}
            options={audioOptions}
            onchange={(audio) => (settings.video.audio = audio)}
            ariaLabel={t(locale, "batch.settings.audio")}
          />
        </div>

        {#if settings.video.audio === "aac"}
          <label class="field-label">
            {t(locale, "batch.settings.audioBitrate")}
            <input
              type="number"
              min="64"
              max="320"
              step="32"
              bind:value={settings.video.audio_bitrate_kbps}
            />
          </label>
        {/if}
      {/if}

      <Switch
        bind:checked={settings.video.faststart}
        label={t(locale, "batch.settings.faststart")}
      />
      <Switch
        bind:checked={settings.video.keep_metadata}
        label={t(locale, "batch.settings.keepMetadata")}
      />
    </div>
  {/if}

  {#if hasImages && (tab === "image" || !hasVideos)}
    <div class="options-grid">
      <div class="field-label narrow">
        {t(locale, "batch.settings.format")}
        <Select
          value={settings.image.format}
          options={formatOptions}
          onchange={(imageFormat) => (settings.image.format = imageFormat)}
          ariaLabel={t(locale, "batch.settings.format")}
        />
      </div>

      {#if settings.image.format !== "png"}
        <label class="field-label">
          {format(locale, "batch.settings.quality", { value: settings.image.quality })}
          <input type="range" min="30" max="100" bind:value={settings.image.quality} />
        </label>
      {/if}

      <div class="field-label narrow">
        {t(locale, "batch.settings.maxEdge")}
        <Select
          value={settings.image.max_edge ?? 0}
          options={edgeOptions}
          onchange={(edge) => (settings.image.max_edge = edge || null)}
          ariaLabel={t(locale, "batch.settings.maxEdge")}
        />
      </div>

      <Switch
        bind:checked={settings.image.keep_metadata}
        label={t(locale, "batch.settings.keepMetadata")}
      />
    </div>
  {/if}

  <h3 class="batch-section-title">{t(locale, "batch.settings.outputTitle")}</h3>
  <div class="options-grid">
    <label class="radio-row">
      <input
        type="radio"
        name="batch-output"
        checked={outputMode === "subfolder"}
        onchange={() => chooseOutput("subfolder")}
      />
      <span>{t(locale, "batch.settings.outputSubfolder")}</span>
    </label>
    {#if outputMode === "subfolder"}
      <label class="field-label narrow indented">
        {t(locale, "batch.settings.subfolderName")}
        <input
          type="text"
          value={subfolderName}
          oninput={(event) => setSubfolderName(event.currentTarget.value)}
        />
      </label>
    {/if}

    <label class="radio-row">
      <input
        type="radio"
        name="batch-output"
        checked={outputMode === "custom_folder"}
        onchange={() => chooseOutput("custom_folder")}
      />
      <span>{t(locale, "batch.settings.outputCustom")}</span>
    </label>
    {#if outputMode === "custom_folder"}
      <div class="field-label indented">
        <button type="button" class="ghost-btn" onclick={onPickOutputFolder}>
          {t(locale, "batch.settings.chooseFolder")}
        </button>
        <small class="option-hint">
          {customFolder || t(locale, "batch.settings.noFolderChosen")}
        </small>
      </div>
    {/if}

    <label class="radio-row danger-option">
      <input
        type="radio"
        name="batch-output"
        checked={outputMode === "replace_original"}
        onchange={() => chooseOutput("replace_original")}
      />
      <span>{t(locale, "batch.settings.outputReplace")}</span>
    </label>

    <label class="field-label narrow">
      {t(locale, "batch.settings.suffix")}
      <input
        type="text"
        value={settings.name_suffix ?? ""}
        oninput={(event) => (settings.name_suffix = event.currentTarget.value || null)}
      />
    </label>

    <div class="field-label narrow">
      {t(locale, "batch.settings.conflict")}
      <Select
        value={settings.on_conflict}
        options={conflictOptions}
        onchange={(policy) => (settings.on_conflict = policy)}
        ariaLabel={t(locale, "batch.settings.conflict")}
      />
    </div>

    <Switch
      bind:checked={settings.skip_if_larger}
      label={t(locale, "batch.settings.skipIfLarger")}
    />

    <label class="field-label">
      {t(locale, "batch.settings.minSavings")}
      <input
        type="number"
        min="0"
        max="90"
        value={settings.skip_if_savings_below_pct ?? 0}
        oninput={(event) =>
          (settings.skip_if_savings_below_pct = Number(event.currentTarget.value) || null)}
      />
    </label>

    <label class="field-label">
      {t(locale, "batch.settings.concurrency")}
      <input type="number" min="1" max="8" bind:value={settings.concurrency} />
    </label>

    <Switch
      bind:checked={settings.preserve_timestamps}
      label={t(locale, "batch.settings.preserveTimestamps")}
    />
  </div>
</div>
