<script lang="ts">
  import Select from "./Select.svelte";
  import SupportBlock from "./SupportBlock.svelte";
  import Switch from "./Switch.svelte";
  import { t, type Locale } from "../i18n";
  import type { RenameMode, SortMode, LayoutMode } from "../types";

  let {
    locale,
    open,
    sortMode = $bindable<SortMode>("exif_date"),
    scanRecursive = $bindable(false),
    renameMode = $bindable<RenameMode>("free"),
    layoutMode = $bindable<LayoutMode>("sidebar"),
    videoWithSound = $bindable(false),
    errorLogCount = 0,
    errorLogPath = "",
    onClose,
    onLocaleChange,
  }: {
    locale: Locale;
    open: boolean;
    sortMode?: SortMode;
    scanRecursive?: boolean;
    renameMode?: RenameMode;
    layoutMode?: LayoutMode;
    videoWithSound?: boolean;
    errorLogCount?: number;
    errorLogPath?: string;
    onClose: () => void;
    onLocaleChange: (locale: Locale) => void;
  } = $props();

  const sortOptions = $derived([
    { value: "exif_date" as SortMode, label: t(locale, "options.sortExif") },
    { value: "file_name" as SortMode, label: t(locale, "options.sortName") },
    { value: "modified_date" as SortMode, label: t(locale, "options.sortModified") },
  ]);

  const renameOptions = $derived([
    { value: "free" as RenameMode, label: t(locale, "options.renameFree") },
    { value: "prefix_counter" as RenameMode, label: t(locale, "options.renamePrefix") },
  ]);

  const layoutOptions = $derived([
    { value: "sidebar" as LayoutMode, label: t(locale, "options.layoutSidebar") },
    { value: "bottom" as LayoutMode, label: t(locale, "options.layoutBottom") },
  ]);
</script>

{#if open}
  <div class="modal-backdrop">
    <button
      type="button"
      class="modal-scrim"
      aria-label={t(locale, "common.close")}
      onclick={onClose}
    ></button>
    <div
      class="options-card"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <h2>{t(locale, "options.title")}</h2>
      <div class="options-grid">
        <div class="field-label">
          {t(locale, "options.sort")}
          <Select
            value={sortMode}
            options={sortOptions}
            onchange={(mode) => (sortMode = mode)}
            ariaLabel={t(locale, "options.sort")}
          />
        </div>

        <Switch
          bind:checked={scanRecursive}
          label={t(locale, "options.recursive")}
          hint={t(locale, "options.recursiveHint")}
        />

        <Switch
          bind:checked={videoWithSound}
          label={t(locale, "options.videoSound")}
          hint={t(locale, "options.videoSoundHint")}
        />

        <div class="field-label">
          {t(locale, "options.renameMode")}
          <Select
            value={renameMode}
            options={renameOptions}
            onchange={(mode) => (renameMode = mode)}
            ariaLabel={t(locale, "options.renameMode")}
          />
        </div>

        <div class="field-label">
          {t(locale, "options.layout")}
          <Select
            value={layoutMode}
            options={layoutOptions}
            onchange={(mode) => (layoutMode = mode)}
            ariaLabel={t(locale, "options.layout")}
          />
        </div>

        <div class="field-label">
          {t(locale, "options.language")}
          <div class="locale-switch">
            <button type="button" class:active={locale === "en"} onclick={() => onLocaleChange("en")}>EN</button>
            <button type="button" class:active={locale === "es"} onclick={() => onLocaleChange("es")}>ES</button>
          </div>
        </div>

        {#if errorLogCount > 0}
          <div class="error-log-box">
            <strong>{t(locale, "options.errorLog")} ({errorLogCount})</strong>
            <p>{t(locale, "options.errorLogHint")}</p>
            <code class="error-log-path">{errorLogPath}</code>
          </div>
        {/if}
      </div>

      <SupportBlock {locale} />

      <div class="modal-actions">
        <button type="button" class="primary-btn" onclick={onClose}>{t(locale, "common.ok")}</button>
      </div>
    </div>
  </div>
{/if}
