<script lang="ts">
  let {
    checked = $bindable(false),
    label,
    hint = "",
    disabled = false,
    onchange,
  }: {
    checked?: boolean;
    label: string;
    hint?: string;
    disabled?: boolean;
    /// For switches whose flip has to run work (a re-scan, a save). Callers
    /// that only need the value keep using `bind:checked`.
    onchange?: (checked: boolean) => void;
  } = $props();

  function toggle() {
    const next = !checked;
    checked = next;
    onchange?.(next);
  }

  const labelId = $props.id();
</script>

<div class="switch-row">
  <span class="switch-copy" id={labelId}>
    <span class="switch-label">{label}</span>
    {#if hint}
      <small class="option-hint">{hint}</small>
    {/if}
  </span>
  <button
    type="button"
    role="switch"
    class="switch"
    class:on={checked}
    aria-checked={checked}
    aria-labelledby={labelId}
    {disabled}
    onclick={toggle}
  >
    <span class="switch-knob"></span>
  </button>
</div>
