<script lang="ts">
  import Select from "./Select.svelte";
  import { builtInPresetsFor } from "../batch";
  import { t, type Locale } from "../i18n";
  import type { BatchSettings, ImageFormat, MediaItem } from "../types";

  let {
    locale,
    item,
    settings = $bindable<BatchSettings>(),
    activePresetId,
    disabled = false,
    onOptimize,
    onAdvanced,
  }: {
    locale: Locale;
    item: MediaItem | null | undefined;
    settings: BatchSettings;
    activePresetId: string | null;
    disabled?: boolean;
    onOptimize: () => void;
    onAdvanced: () => void;
  } = $props();

  const isVideo = $derived(!!item?.is_video);
  const presets = $derived(builtInPresetsFor(locale, isVideo ? "video" : "image"));

  const heightOptions = $derived([
    { value: 0, label: t(locale, "batch.settings.keepResolution") },
    { value: 720, label: "720p" },
    { value: 1080, label: "1080p" },
    { value: 1440, label: "1440p" },
    { value: 2160, label: "2160p (4K)" },
  ]);

  const formatOptions = $derived([
    { value: "jpeg" as ImageFormat, label: t(locale, "batch.settings.formatJpeg") },
    { value: "webp" as ImageFormat, label: t(locale, "batch.settings.formatWebp") },
    { value: "png" as ImageFormat, label: t(locale, "batch.settings.formatPng") },
    { value: "keep" as ImageFormat, label: t(locale, "batch.settings.formatKeep") },
  ]);
</script>

<div class="quick-optimize">
  <span class="field-label">{t(locale, "batch.settings.presets")}</span>
  <div class="quick-preset-list">
    {#each presets as preset (preset.id)}
      <button
        type="button"
        class="quick-preset"
        class:active={preset.id === activePresetId}
        aria-pressed={preset.id === activePresetId}
        {disabled}
        onclick={() => (settings = { ...settings, ...preset.settings })}
      >
        {preset.name}
      </button>
    {/each}
  </div>

  {#if isVideo}
    <div class="field-label">
      {t(locale, "batch.settings.maxHeight")}
      <Select
        value={settings.video.max_height ?? 0}
        options={heightOptions}
        {disabled}
        onchange={(height) => (settings.video.max_height = height || null)}
        ariaLabel={t(locale, "batch.settings.maxHeight")}
      />
    </div>
  {:else}
    <div class="field-label">
      {t(locale, "batch.settings.format")}
      <Select
        value={settings.image.format}
        options={formatOptions}
        {disabled}
        onchange={(imageFormat) => (settings.image.format = imageFormat)}
        ariaLabel={t(locale, "batch.settings.format")}
      />
    </div>
  {/if}

  <div class="quick-actions">
    <button type="button" class="primary-btn" disabled={disabled || !item} onclick={onOptimize}>
      {t(locale, "sidePanel.optimizeNow")}
    </button>
    <button type="button" class="ghost-btn" disabled={!item} onclick={onAdvanced}>
      {t(locale, "sidePanel.advanced")}
    </button>
  </div>
  <small class="option-hint">{t(locale, "sidePanel.optimizeFileHint")}</small>
</div>
