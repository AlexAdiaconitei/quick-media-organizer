<script lang="ts">
  /// A yes/no prompt in the app's own chrome.
  ///
  /// `window.confirm` blocks the whole webview and ignores the app's styling
  /// and language, so every question the UI asks goes through here instead.
  import { t, type Locale } from "../i18n";

  let {
    locale,
    open,
    message,
    confirmLabel,
    cancelLabel,
    onConfirm,
    onCancel,
  }: {
    locale: Locale;
    open: boolean;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();

  let confirmButton = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (open) queueMicrotask(() => confirmButton?.focus());
  });

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onCancel();
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      event.stopPropagation();
      onConfirm();
    }
  }
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
  <div class="modal-backdrop">
    <button
      type="button"
      class="modal-scrim"
      aria-label={cancelLabel ?? t(locale, "common.cancel")}
      onclick={onCancel}
    ></button>
    <div
      class="modal-card confirm-card"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="confirm-dialog-message"
      tabindex="-1"
    >
      <p id="confirm-dialog-message">{message}</p>

      <div class="modal-actions">
        <button type="button" class="ghost-btn" onclick={onCancel}>
          {cancelLabel ?? t(locale, "common.cancel")}
        </button>
        <button
          type="button"
          class="primary-btn"
          bind:this={confirmButton}
          onclick={onConfirm}
        >
          {confirmLabel ?? t(locale, "common.ok")}
        </button>
      </div>
    </div>
  </div>
{/if}
