<script lang="ts">
  import { openExternal } from "../links";
  import { format, t, type Locale } from "../i18n";
  import type { AvailableUpdate } from "../updater";

  let {
    locale,
    open,
    update,
    releasesUrl = null,
    installing = false,
    progress = 0,
    onInstall,
    onClose,
  }: {
    locale: Locale;
    open: boolean;
    update: AvailableUpdate | null;
    releasesUrl?: string | null;
    installing?: boolean;
    progress?: number;
    onInstall: () => void;
    onClose: () => void;
  } = $props();

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !installing) {
      event.preventDefault();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open && update}
  <div class="modal-backdrop">
    <button
      type="button"
      class="modal-scrim"
      aria-label={t(locale, "common.close")}
      onclick={installing ? () => {} : onClose}
    ></button>
    <div
      class="modal-card update-card"
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-title"
      tabindex="-1"
    >
      <h2 id="update-title">{t(locale, "update.title")}</h2>
      <p class="update-versions">
        {format(locale, "update.versions", {
          current: update.currentVersion,
          next: update.version,
        })}
      </p>

      {#if update.notes}
        <div class="update-notes">
          <span class="field-label">{t(locale, "update.notes")}</span>
          <pre>{update.notes}</pre>
        </div>
      {:else}
        <p class="option-hint">{t(locale, "update.noNotes")}</p>
      {/if}

      {#if installing}
        <div class="update-progress">
          <div class="progress-bar">
            <div class="progress-fill" style={`width:${Math.round(progress * 100)}%`}></div>
          </div>
          <p class="option-hint">{t(locale, "update.installing")}</p>
        </div>
      {/if}

      <div class="modal-actions">
        <button type="button" class="primary-btn" disabled={installing} onclick={onInstall}>
          {t(locale, "update.install")}
        </button>
        {#if releasesUrl}
          <button
            type="button"
            class="ghost-btn"
            onclick={() => void openExternal(releasesUrl)}
          >
            {t(locale, "update.viewRelease")}
          </button>
        {/if}
        <button type="button" class="ghost-btn" disabled={installing} onclick={onClose}>
          {t(locale, "update.later")}
        </button>
      </div>
    </div>
  </div>
{/if}
