<script lang="ts" generics="T extends string | number">
  /// The native dropdown list is drawn by the OS and cannot be themed, so the
  /// options are rendered as a listbox instead.
  let {
    value,
    options,
    onchange,
    disabled = false,
    ariaLabel = "",
    width = "",
  }: {
    value: T;
    options: { value: T; label: string }[];
    onchange: (value: T) => void;
    disabled?: boolean;
    ariaLabel?: string;
    /// Optional CSS width, for fields that should not stretch the panel.
    width?: string;
  } = $props();

  let open = $state(false);
  let highlighted = $state(0);
  let root = $state<HTMLDivElement | null>(null);
  let trigger = $state<HTMLButtonElement | null>(null);

  const selectedIndex = $derived(
    Math.max(
      0,
      options.findIndex((option) => option.value === value),
    ),
  );
  const selectedLabel = $derived(options[selectedIndex]?.label ?? "");
  const listId = $props.id();

  function openList() {
    if (disabled) return;
    highlighted = selectedIndex;
    open = true;
  }

  function close(focusTrigger = true) {
    open = false;
    if (focusTrigger) trigger?.focus();
  }

  function choose(index: number) {
    const option = options[index];
    if (!option) return;
    onchange(option.value);
    close();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (!open) {
      if (event.key === "ArrowDown" || event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        openList();
      }
      return;
    }

    switch (event.key) {
      case "Escape":
        event.preventDefault();
        event.stopPropagation();
        close();
        break;
      case "ArrowDown":
        event.preventDefault();
        highlighted = Math.min(highlighted + 1, options.length - 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        highlighted = Math.max(highlighted - 1, 0);
        break;
      case "Home":
        event.preventDefault();
        highlighted = 0;
        break;
      case "End":
        event.preventDefault();
        highlighted = options.length - 1;
        break;
      case "Enter":
      case " ":
        event.preventDefault();
        choose(highlighted);
        break;
      case "Tab":
        close(false);
        break;
    }
  }

  function handlePointerDown(event: PointerEvent) {
    if (!open || !root) return;
    if (!root.contains(event.target as Node)) close(false);
  }
</script>

<svelte:window onpointerdown={open ? handlePointerDown : undefined} />

<div class="select" bind:this={root} style={width ? `width:${width}` : undefined}>
  <button
    type="button"
    class="select-trigger"
    class:open
    bind:this={trigger}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-label={ariaLabel || undefined}
    {disabled}
    onclick={() => (open ? close() : openList())}
    onkeydown={handleKeydown}
  >
    <span class="select-value">{selectedLabel}</span>
    <span class="select-chevron" aria-hidden="true"></span>
  </button>

  {#if open}
    <ul class="select-list" role="listbox" id={listId} aria-label={ariaLabel || undefined}>
      {#each options as option, index (option.value)}
        <li
          role="option"
          aria-selected={option.value === value}
          class:highlighted={index === highlighted}
          class:selected={option.value === value}
        >
          <button
            type="button"
            tabindex="-1"
            onclick={() => choose(index)}
            onmouseenter={() => (highlighted = index)}
          >
            <span class="select-check" aria-hidden="true"></span>
            <span>{option.label}</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>
