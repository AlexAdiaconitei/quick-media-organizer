<script lang="ts">
  let {
    message,
    error = false,
    onDismiss,
  }: {
    message: string;
    error?: boolean;
    onDismiss?: () => void;
  } = $props();
</script>

{#if message}
  <div class="toast" class:error role="alert" aria-live="assertive">
    <span>{message}</span>
    {#if onDismiss}
      <button type="button" class="toast-dismiss" onclick={onDismiss} aria-label="Dismiss">×</button>
    {/if}
  </div>
{/if}
