<script lang="ts">
  import { LINKS, openExternal } from "../links";
  import { t, type Locale } from "../i18n";

  let {
    locale,
    compact = false,
    repositoryUrl = null,
  }: {
    locale: Locale;
    compact?: boolean;
    /// Repository this build came from; hidden when it cannot be resolved.
    repositoryUrl?: string | null;
  } = $props();
</script>

<div class="support-block" class:support-block-compact={compact}>
  <p class="support-message">{t(locale, "support.message")}</p>
  <div class="support-actions">
    <button
      type="button"
      class="support-btn support-btn-primary"
      onclick={() => void openExternal(LINKS.buyMeACoffee)}
    >
      ☕ {t(locale, "support.coffee")}
    </button>
    {#if !compact}
      <button
        type="button"
        class="support-btn"
        onclick={() => void openExternal(LINKS.email)}
      >
        {t(locale, "support.email")}
      </button>
      <button
        type="button"
        class="support-btn"
        onclick={() => void openExternal(LINKS.linkedIn)}
      >
        {t(locale, "support.linkedin")}
      </button>
    {/if}
    {#if repositoryUrl}
      <button
        type="button"
        class="support-btn"
        onclick={() => void openExternal(repositoryUrl)}
      >
        {t(locale, "support.github")}
      </button>
    {/if}
  </div>
</div>
