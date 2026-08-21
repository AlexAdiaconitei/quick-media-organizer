<script lang="ts">
  import { t, type Locale } from "../i18n";

  let {
    locale,
    open,
    query = $bindable(""),
    selected = $bindable<string | null>(null),
    favorites,
    recent,
    existing,
    onConfirm,
    onClose,
    onToggleFavorite,
  }: {
    locale: Locale;
    open: boolean;
    query?: string;
    selected?: string | null;
    favorites: string[];
    recent: string[];
    existing: string[];
    onConfirm: () => void;
    onClose: () => void;
    onToggleFavorite: (folder: string) => void;
  } = $props();

  let searchInput = $state<HTMLInputElement | null>(null);
  let highlightedIndex = $state(0);
  let optionRefs = $state<(HTMLButtonElement | null)[]>([]);

  const pool = $derived([...new Set([...recent, ...favorites, ...existing])]);

  const trimmedQuery = $derived(query.trim());
  const normalizedQuery = $derived(trimmedQuery.toLowerCase());
  const isFiltering = $derived(normalizedQuery.length > 0);

  function folderMatchesPrefix(folder: string, q: string): boolean {
    const lower = folder.toLowerCase();
    if (lower.startsWith(q)) return true;
    return lower.split("/").some((segment) => segment.startsWith(q));
  }

  const prefixMatches = $derived.by(() => {
    if (!normalizedQuery) return [];
    return pool
      .filter((item) => folderMatchesPrefix(item, normalizedQuery))
      .sort((a, b) => {
        const aLower = a.toLowerCase();
        const bLower = b.toLowerCase();
        const aExact = aLower.startsWith(normalizedQuery) ? 0 : 1;
        const bExact = bLower.startsWith(normalizedQuery) ? 0 : 1;
        return aExact - bExact || a.length - b.length || a.localeCompare(b);
      })
      .slice(0, 12);
  });

  const browseSuggestions = $derived(pool.slice(0, 12));

  const activeMatches = $derived(isFiltering ? prefixMatches : browseSuggestions);

  const effectiveFolder = $derived(trimmedQuery || selected || "");
  const activeCompletion = $derived(
    activeMatches[highlightedIndex] ?? activeMatches[0] ?? null,
  );

  $effect(() => {
    if (open) {
      highlightedIndex = 0;
      queueMicrotask(() => {
        searchInput?.focus();
        searchInput?.select();
      });
    }
  });

  $effect(() => {
    query;
    highlightedIndex = 0;
  });

  $effect(() => {
    highlightedIndex;
    activeMatches;
    queueMicrotask(() => {
      optionRefs[highlightedIndex]?.scrollIntoView({ block: "nearest" });
    });
  });

  function pick(folder: string) {
    selected = folder;
    query = folder;
  }

  function confirmSelection() {
    const folder = trimmedQuery || selected || "";
    if (!folder) return;
    selected = folder;
    query = folder;
    onConfirm();
  }

  function acceptCompletion() {
    const match = activeCompletion;
    if (!match || !normalizedQuery) return false;
    if (match.toLowerCase() === normalizedQuery) return false;
    pick(match);
    return true;
  }

  function handleInputKeydown(event: KeyboardEvent) {
    if (event.key === "Tab" && !event.shiftKey) {
      if (acceptCompletion()) {
        event.preventDefault();
      }
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      confirmSelection();
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key === "ArrowDown") {
      if (activeMatches.length === 0) return;
      event.preventDefault();
      highlightedIndex = Math.min(highlightedIndex + 1, activeMatches.length - 1);
      return;
    }

    if (event.key === "ArrowUp") {
      if (activeMatches.length === 0) return;
      event.preventDefault();
      highlightedIndex = Math.max(highlightedIndex - 1, 0);
    }
  }

  function handleBackdropClick() {
    onClose();
  }

  function isHighlighted(index: number) {
    return index === highlightedIndex;
  }
</script>

{#if open}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={handleBackdropClick}
    onkeydown={(event) => {
      if (event.key === "Escape") onClose();
    }}
  >
    <div
      class="modal-card folder-picker-card"
      role="dialog"
      aria-modal="true"
      aria-labelledby="folder-picker-title"
      tabindex="-1"
    >
      <h2 id="folder-picker-title">{t(locale, "folderPicker.title")}</h2>

      <div class="folder-picker-search">
        <label class="field-label" for="folder-picker-input">
          {t(locale, "folderPicker.search")}
        </label>
        <input
          bind:this={searchInput}
          id="folder-picker-input"
          class="rename-input"
          bind:value={query}
          autocomplete="off"
          spellcheck="false"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={isFiltering && activeMatches.length > 0}
          aria-controls="folder-picker-suggestions"
          placeholder={t(locale, "folderPicker.createHint")}
          onkeydown={handleInputKeydown}
        />

        {#if isFiltering}
          <div
            class="folder-suggestions-dropdown"
            id="folder-picker-suggestions"
            role="listbox"
            aria-label={t(locale, "folderPicker.matches")}
          >
            {#if activeMatches.length}
              {#each activeMatches as folder, index}
                <button
                  bind:this={optionRefs[index]}
                  type="button"
                  class="folder-suggestion-item"
                  class:highlighted={isHighlighted(index)}
                  role="option"
                  aria-selected={isHighlighted(index)}
                  onclick={() => pick(folder)}
                  onmouseenter={() => {
                    highlightedIndex = index;
                  }}
                >
                  {folder}
                </button>
              {/each}
            {:else}
              <div class="folder-suggestion-empty">
                {t(locale, "folderPicker.noMatches")}
              </div>
            {/if}
          </div>
        {/if}

        <small class="folder-picker-hint">{t(locale, "folderPicker.keyboardHint")}</small>
      </div>

      {#if !isFiltering && favorites.length}
        <h3>{t(locale, "folderPicker.favorites")}</h3>
        <div class="folder-list">
          {#each favorites as folder}
            <button
              type="button"
              class="folder-item"
              class:selected={selected === folder || query === folder}
              onclick={() => pick(folder)}
            >
              ⭐ {folder}
            </button>
          {/each}
        </div>
      {/if}

      {#if !isFiltering && recent.length}
        <h3>{t(locale, "folderPicker.recent")}</h3>
        <div class="folder-list">
          {#each recent as folder}
            <button
              type="button"
              class="folder-item"
              class:selected={selected === folder || query === folder}
              onclick={() => pick(folder)}
            >
              {folder}
            </button>
          {/each}
        </div>
      {/if}

      {#if !isFiltering && browseSuggestions.length}
        <h3>{t(locale, "folderPicker.existing")}</h3>
        <div class="folder-list">
          {#each browseSuggestions as folder}
            <button
              type="button"
              class="folder-item"
              class:selected={selected === folder || query === folder}
              onclick={() => pick(folder)}
            >
              {folder}
            </button>
          {/each}
        </div>
      {/if}

      <div class="modal-actions">
        <button
          type="button"
          class="primary-btn"
          disabled={!effectiveFolder}
          onclick={confirmSelection}
        >
          {t(locale, "folderPicker.confirm")}
        </button>
        <button type="button" class="ghost-btn" onclick={onClose}>{t(locale, "folderPicker.cancel")}</button>
        {#if effectiveFolder}
          <button type="button" class="ghost-btn" onclick={() => onToggleFavorite(effectiveFolder)}>⭐</button>
        {/if}
      </div>
    </div>
  </div>
{/if}
