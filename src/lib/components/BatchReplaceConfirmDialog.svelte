<script lang="ts">
  import { format, t, type Locale } from "../i18n";
  import { modLabel } from "../shortcuts";
  import { formatSize } from "../batch";

  let {
    locale,
    open,
    fileCount,
    totalBytes,
    backupPath,
    onConfirm,
    onCancel,
  }: {
    locale: Locale;
    open: boolean;
    fileCount: number;
    totalBytes: number;
    backupPath: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();

  let acknowledged = $state(false);
  let cancelButton = $state<HTMLButtonElement | null>(null);

  // Reset on every open: consent is never carried over from a previous run.
  $effect(() => {
    if (open) {
      acknowledged = false;
      queueMicrotask(() => cancelButton?.focus());
    }
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
  <div class="modal-backdrop danger-backdrop" role="presentation" onclick={onCancel}>
    <div
      class="modal-card replace-confirm-card"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="replace-confirm-title"
      tabindex="-1"
      onclick={(event) => event.stopPropagation()}
    >
      <h2 id="replace-confirm-title">{t(locale, "batch.replaceConfirm.title")}</h2>
      <p class="replace-summary">
        {format(locale, "batch.replaceConfirm.summary", {
          count: fileCount,
          size: formatSize(totalBytes),
        })}
      </p>

      <ul class="replace-points">
        <li>{format(locale, "batch.replaceConfirm.point1", { path: backupPath })}</li>
        <li>{t(locale, "batch.replaceConfirm.point2")}</li>
        <li>{t(locale, "batch.replaceConfirm.point3")}</li>
        <li>{t(locale, "batch.replaceConfirm.point4")}</li>
        <li>{format(locale, "batch.replaceConfirm.point5", { key: modLabel("Z") })}</li>
        <li>{t(locale, "batch.replaceConfirm.point6")}</li>
      </ul>

      <label class="checkbox-row acknowledge-row">
        <input type="checkbox" bind:checked={acknowledged} />
        <span>{t(locale, "batch.replaceConfirm.acknowledge")}</span>
      </label>

      <div class="modal-actions">
        <button
          type="button"
          class="primary-btn"
          bind:this={cancelButton}
          onclick={onCancel}
        >
          {t(locale, "batch.replaceConfirm.cancel")}
        </button>
        <button
          type="button"
          class="danger-btn"
          disabled={!acknowledged}
          onclick={onConfirm}
        >
          {t(locale, "batch.replaceConfirm.confirm")}
        </button>
      </div>
    </div>
  </div>
{/if}
