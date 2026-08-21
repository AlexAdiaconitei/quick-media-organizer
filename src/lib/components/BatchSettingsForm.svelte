<script lang="ts">
  import { format, t, type Locale } from "../i18n";
  import type { BatchPreset, BatchSettings, FfmpegCapabilities } from "../types";

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

  function numberOrNull(value: string): number | null {
    const parsed = Number(value);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
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
      <label class="field-label">
        {t(locale, "batch.settings.codec")}
        <select bind:value={settings.video.codec}>
          {#if capabilities.h265}
            <option value="h265">{t(locale, "batch.settings.codecH265")}</option>
          {/if}
          {#if capabilities.h264}
            <option value="h264">{t(locale, "batch.settings.codecH264")}</option>
          {/if}
          {#if capabilities.av1}
            <option value="av1">{t(locale, "batch.settings.codecAv1")}</option>
          {/if}
          <option value="copy">{t(locale, "batch.settings.codecCopy")}</option>
        </select>
      </label>

      {#if settings.video.codec !== "copy"}
        <label class="field-label">
          {format(locale, "batch.settings.crf", { value: settings.video.crf })}
          <input type="range" min="16" max="35" bind:value={settings.video.crf} />
          <small class="option-hint">{t(locale, "batch.settings.crfHint")}</small>
        </label>

        <label class="field-label">
          {t(locale, "batch.settings.speed")}
          <select bind:value={settings.video.speed_preset}>
            <option value="slow">{t(locale, "batch.settings.speedSlow")}</option>
            <option value="medium">{t(locale, "batch.settings.speedMedium")}</option>
            <option value="fast">{t(locale, "batch.settings.speedFast")}</option>
          </select>
        </label>

        <label class="field-label">
          {t(locale, "batch.settings.maxHeight")}
          <select
            value={settings.video.max_height ?? ""}
            onchange={(event) =>
              (settings.video.max_height = numberOrNull(event.currentTarget.value))}
          >
            <option value="">{t(locale, "batch.settings.keepResolution")}</option>
            <option value="720">720p</option>
            <option value="1080">1080p</option>
            <option value="1440">1440p</option>
            <option value="2160">2160p (4K)</option>
          </select>
        </label>

        <label class="field-label">
          {t(locale, "batch.settings.maxFps")}
          <select
            value={settings.video.max_fps ?? ""}
            onchange={(event) =>
              (settings.video.max_fps = numberOrNull(event.currentTarget.value))}
          >
            <option value="">{t(locale, "batch.settings.keepFps")}</option>
            <option value="24">24</option>
            <option value="30">30</option>
            <option value="60">60</option>
          </select>
        </label>

        <label class="field-label">
          {t(locale, "batch.settings.audio")}
          <select bind:value={settings.video.audio}>
            <option value="aac">{t(locale, "batch.settings.audioAac")}</option>
            <option value="copy">{t(locale, "batch.settings.audioCopy")}</option>
            <option value="drop">{t(locale, "batch.settings.audioDrop")}</option>
          </select>
        </label>

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

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={settings.video.faststart} />
        <span>{t(locale, "batch.settings.faststart")}</span>
      </label>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={settings.video.keep_metadata} />
        <span>{t(locale, "batch.settings.keepMetadata")}</span>
      </label>
    </div>
  {/if}

  {#if hasImages && (tab === "image" || !hasVideos)}
    <div class="options-grid">
      <label class="field-label">
        {t(locale, "batch.settings.format")}
        <select bind:value={settings.image.format}>
          <option value="jpeg">{t(locale, "batch.settings.formatJpeg")}</option>
          {#if capabilities.webp}
            <option value="webp">{t(locale, "batch.settings.formatWebp")}</option>
          {/if}
          {#if capabilities.avif}
            <option value="avif">{t(locale, "batch.settings.formatAvif")}</option>
          {/if}
          <option value="png">{t(locale, "batch.settings.formatPng")}</option>
          <option value="keep">{t(locale, "batch.settings.formatKeep")}</option>
        </select>
      </label>

      {#if settings.image.format !== "png"}
        <label class="field-label">
          {format(locale, "batch.settings.quality", { value: settings.image.quality })}
          <input type="range" min="30" max="100" bind:value={settings.image.quality} />
        </label>
      {/if}

      <label class="field-label">
        {t(locale, "batch.settings.maxEdge")}
        <select
          value={settings.image.max_edge ?? ""}
          onchange={(event) =>
            (settings.image.max_edge = numberOrNull(event.currentTarget.value))}
        >
          <option value="">{t(locale, "batch.settings.keepSize")}</option>
          <option value="1920">1920</option>
          <option value="2560">2560</option>
          <option value="3840">3840</option>
        </select>
      </label>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={settings.image.keep_metadata} />
        <span>{t(locale, "batch.settings.keepMetadata")}</span>
      </label>
    </div>
  {/if}

  <h3 class="batch-section-title">{t(locale, "batch.settings.outputTitle")}</h3>
  <div class="options-grid">
    <label class="checkbox-row">
      <input
        type="radio"
        name="batch-output"
        checked={outputMode === "subfolder"}
        onchange={() => chooseOutput("subfolder")}
      />
      <span>{t(locale, "batch.settings.outputSubfolder")}</span>
    </label>
    {#if outputMode === "subfolder"}
      <label class="field-label indented">
        {t(locale, "batch.settings.subfolderName")}
        <input
          type="text"
          value={subfolderName}
          oninput={(event) => setSubfolderName(event.currentTarget.value)}
        />
      </label>
    {/if}

    <label class="checkbox-row">
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

    <label class="checkbox-row danger-option">
      <input
        type="radio"
        name="batch-output"
        checked={outputMode === "replace_original"}
        onchange={() => chooseOutput("replace_original")}
      />
      <span>{t(locale, "batch.settings.outputReplace")}</span>
    </label>

    <label class="field-label">
      {t(locale, "batch.settings.suffix")}
      <input
        type="text"
        value={settings.name_suffix ?? ""}
        oninput={(event) => (settings.name_suffix = event.currentTarget.value || null)}
      />
    </label>

    <label class="field-label">
      {t(locale, "batch.settings.conflict")}
      <select bind:value={settings.on_conflict}>
        <option value="rename">{t(locale, "batch.settings.conflictRename")}</option>
        <option value="skip">{t(locale, "batch.settings.conflictSkip")}</option>
        <option value="overwrite">{t(locale, "batch.settings.conflictOverwrite")}</option>
      </select>
    </label>

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={settings.skip_if_larger} />
      <span>{t(locale, "batch.settings.skipIfLarger")}</span>
    </label>

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

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={settings.preserve_timestamps} />
      <span>{t(locale, "batch.settings.preserveTimestamps")}</span>
    </label>
  </div>
</div>
