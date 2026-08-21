<script lang="ts">
  import SupportBlock from "./SupportBlock.svelte";
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
        <label class="field-label">
          {t(locale, "options.sort")}
          <select bind:value={sortMode}>
            <option value="exif_date">{t(locale, "options.sortExif")}</option>
            <option value="file_name">{t(locale, "options.sortName")}</option>
            <option value="modified_date">{t(locale, "options.sortModified")}</option>
          </select>
        </label>

        <label class="checkbox-row">
          <input type="checkbox" bind:checked={scanRecursive} />
          <span>
            {t(locale, "options.recursive")}
            <small class="option-hint">{t(locale, "options.recursiveHint")}</small>
          </span>
        </label>

        <label class="checkbox-row">
          <input type="checkbox" bind:checked={videoWithSound} />
          <span>
            {t(locale, "options.videoSound")}
            <small class="option-hint">{t(locale, "options.videoSoundHint")}</small>
          </span>
        </label>

        <label class="field-label">
          {t(locale, "options.renameMode")}
          <select bind:value={renameMode}>
            <option value="free">{t(locale, "options.renameFree")}</option>
            <option value="prefix_counter">{t(locale, "options.renamePrefix")}</option>
          </select>
        </label>

        <label class="field-label">
          {t(locale, "options.layout")}
          <select bind:value={layoutMode}>
            <option value="sidebar">{t(locale, "options.layoutSidebar")}</option>
            <option value="bottom">{t(locale, "options.layoutBottom")}</option>
          </select>
        </label>

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
