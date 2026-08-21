<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { format, t, type Locale } from "../i18n";
  import { formatSize } from "../batch";
  import type { MediaItem } from "../types";

  let {
    locale,
    items,
    selected = $bindable<Set<string>>(new Set()),
    busy = false,
    demoMode = false,
  }: {
    locale: Locale;
    items: MediaItem[];
    selected?: Set<string>;
    busy?: boolean;
    demoMode?: boolean;
  } = $props();

  let lastToggled = $state<number | null>(null);

  const totalBytes = $derived(
    items
      .filter((item) => selected.has(item.id))
      .reduce((sum, item) => sum + item.size_bytes, 0),
  );

  function apply(next: Set<string>) {
    // Reassign so Svelte sees a new reference.
    selected = next;
  }

  function toggle(item: MediaItem, index: number, event: MouseEvent) {
    const next = new Set(selected);

    if (event.shiftKey && lastToggled !== null) {
      const [from, to] = index < lastToggled ? [index, lastToggled] : [lastToggled, index];
      const turningOn = !next.has(item.id);
      for (let i = from; i <= to; i += 1) {
        if (turningOn) next.add(items[i].id);
        else next.delete(items[i].id);
      }
    } else if (next.has(item.id)) {
      next.delete(item.id);
    } else {
      next.add(item.id);
    }

    lastToggled = index;
    apply(next);
  }

  function selectAll() {
    apply(new Set(items.map((item) => item.id)));
  }

  function selectNone() {
    apply(new Set());
  }

  function selectVideos() {
    apply(new Set(items.filter((item) => item.is_video).map((item) => item.id)));
  }

  function selectImages() {
    apply(new Set(items.filter((item) => !item.is_video).map((item) => item.id)));
  }

  function thumbnail(item: MediaItem): string | null {
    if (item.is_video) return null;
    return demoMode ? item.paths[0] : convertFileSrc(item.paths[0]);
  }
</script>

<div class="batch-select">
  <div class="batch-select-toolbar">
    <button type="button" class="ghost-btn" disabled={busy} onclick={selectAll}>
      {t(locale, "batch.select.all")}
    </button>
    <button type="button" class="ghost-btn" disabled={busy} onclick={selectNone}>
      {t(locale, "batch.select.none")}
    </button>
    <button type="button" class="ghost-btn" disabled={busy} onclick={selectVideos}>
      {t(locale, "batch.select.onlyVideos")}
    </button>
    <button type="button" class="ghost-btn" disabled={busy} onclick={selectImages}>
      {t(locale, "batch.select.onlyImages")}
    </button>
    <span class="batch-select-count">
      {format(locale, "batch.select.selected", {
        count: selected.size,
        size: formatSize(totalBytes),
      })}
    </span>
  </div>

  {#if items.length === 0}
    <p class="batch-empty">{t(locale, "batch.select.empty")}</p>
  {:else}
    <div class="batch-grid">
      {#each items as item, index (item.id)}
        <button
          type="button"
          class="batch-tile"
          class:selected={selected.has(item.id)}
          disabled={busy}
          onclick={(event) => toggle(item, index, event)}
          title={item.paths[0]}
        >
          <span class="batch-tile-thumb">
            {#if thumbnail(item)}
              <img src={thumbnail(item)} alt="" loading="lazy" />
            {:else}
              <span class="batch-tile-ext">{item.extension.toUpperCase()}</span>
            {/if}
            {#if item.is_video}
              <span class="batch-tile-badge">{t(locale, "batch.select.videoBadge")}</span>
            {/if}
            {#if item.kind === "live_photo"}
              <span class="batch-tile-badge live">{t(locale, "livePhoto")}</span>
            {/if}
          </span>
          <span class="batch-tile-name">{item.file_name}</span>
          <span class="batch-tile-size">{formatSize(item.size_bytes)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>
