<script lang="ts">
  import Kbd from "./Kbd.svelte";
  import SupportBlock from "./SupportBlock.svelte";
  import { t, type Locale } from "../i18n";
  import { modLabel, skipModLabel } from "../shortcuts";

  let {
    locale,
    open,
    onClose,
  }: {
    locale: Locale;
    open: boolean;
    onClose: () => void;
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
      class="modal-card"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
    >
      <h2>{t(locale, "help.title")}</h2>
      <div class="shortcut-row">
        <Kbd label="Enter" text={t(locale, "shortcuts.enter")} />
        <Kbd label={modLabel("F")} text={t(locale, "shortcuts.folder")} />
        <Kbd label={modLabel("D")} text={t(locale, "shortcuts.delete")} danger />
        <Kbd label={skipModLabel()} text={t(locale, "shortcuts.space")} />
        <Kbd label="← →" text={t(locale, "shortcuts.nav")} />
        <Kbd label={modLabel("Z")} text={t(locale, "shortcuts.undo")} />
        <Kbd label={modLabel("M")} text={t(locale, "shortcuts.info")} />
        <Kbd label={modLabel("B")} text={t(locale, "batch.open")} />
        <Kbd label={modLabel("O")} text={t(locale, "shortcuts.options")} />
        <Kbd label="Esc" text={t(locale, "common.close")} />
      </div>
      <p>{t(locale, "help.faqDeleted")}</p>
      <p>{t(locale, "help.faqDates")}</p>
      <p>{t(locale, "help.modifierHint")}</p>

      <SupportBlock {locale} compact />

      <div class="modal-actions">
        <button type="button" class="primary-btn" onclick={onClose}>{t(locale, "common.ok")}</button>
      </div>
    </div>
  </div>
{/if}
