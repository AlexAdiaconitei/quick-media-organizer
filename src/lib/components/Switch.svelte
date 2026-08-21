<script lang="ts">
  let {
    checked = $bindable(false),
    label,
    hint = "",
    disabled = false,
  }: {
    checked?: boolean;
    label: string;
    hint?: string;
    disabled?: boolean;
  } = $props();

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
    onclick={() => (checked = !checked)}
  >
    <span class="switch-knob"></span>
  </button>
</div>
