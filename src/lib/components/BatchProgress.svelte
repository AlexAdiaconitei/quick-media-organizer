<script lang="ts">
  import { openPath } from "@tauri-apps/plugin-opener";
  import { format, t, type Locale } from "../i18n";
  import { formatFailureReport, formatSize, savingsPercent } from "../batch";
  import type { BatchItemStatus, BatchJobStatus } from "../types";

  let {
    locale,
    job,
    cancelling = false,
    onCancel,
    onError = () => {},
  }: {
    locale: Locale;
    job: BatchJobStatus;
    cancelling?: boolean;
    onCancel: () => void;
    onError?: (message: string) => void;
  } = $props();

  async function openOutputFolder() {
    if (!job.output_dir) return;
    try {
      await openPath(job.output_dir);
    } catch (error) {
      onError(String(error));
    }
  }

  const finished = $derived(job.total - job.items.filter(isPending).length);
  const percent = $derived(job.total === 0 ? 0 : (finished / job.total) * 100);
  const failures = $derived(job.items.filter((item) => item.state === "failed"));
  const savings = $derived(savingsPercent(job.bytes_before, job.bytes_after));
  let failuresCopied = $state(false);

  function isPending(item: BatchItemStatus): boolean {
    return item.state === "pending" || item.state === "running";
  }

  function stateLabel(item: BatchItemStatus): string {
    return t(locale, `batch.run.state${item.state[0].toUpperCase()}${item.state.slice(1)}`);
  }

  async function copyFailures() {
    try {
      await navigator.clipboard.writeText(formatFailureReport(failures));
      failuresCopied = true;
      window.setTimeout(() => (failuresCopied = false), 1800);
    } catch (error) {
      onError(String(error));
    }
  }
</script>

<div class="batch-progress">
  <div class="progress-bar batch-progress-bar">
    <div class="progress-fill" style={`width:${percent}%`}></div>
  </div>
  <div class="batch-progress-head">
    <span>{format(locale, "batch.run.running", { done: finished, total: job.total })}</span>
    {#if job.running}
      <button type="button" class="ghost-btn" disabled={cancelling} onclick={onCancel}>
        {cancelling ? t(locale, "batch.run.cancelling") : t(locale, "batch.run.cancel")}
      </button>
    {/if}
  </div>

  {#if job.running}
    <p class="option-hint">{t(locale, "batch.run.backgroundNotice")}</p>
  {:else}
    <div class="batch-summary">
      <p class="batch-summary-line">
        {format(locale, "batch.run.summary", {
          done: job.done,
          skipped: job.skipped,
          failed: job.failed,
        })}
      </p>
      <p class="batch-summary-savings">
        {#if job.bytes_before > 0 && job.bytes_after < job.bytes_before}
          {format(locale, "batch.run.savings", {
            before: formatSize(job.bytes_before),
            after: formatSize(job.bytes_after),
            percent: savings,
          })}
        {:else}
          {t(locale, "batch.run.noSavings")}
        {/if}
      </p>
      {#if job.cancelled}
        <p class="option-hint">{t(locale, "batch.run.cancelledNotice")}</p>
      {/if}
      {#if job.output_dir}
        <button
          type="button"
          class="ghost-btn"
          onclick={() => void openOutputFolder()}
        >
          {t(locale, "batch.run.openOutput")}
        </button>
      {/if}
    </div>
  {/if}

  <ul class="batch-item-list">
    {#each job.items as item (item.id)}
      <li class="batch-item" data-state={item.state}>
        <span class="batch-item-name" title={item.source_path}>{item.file_name}</span>
        <span class="batch-item-state">
          {#if item.state === "running"}
            {stateLabel(item)} {Math.round(item.progress * 100)}%
          {:else}
            {stateLabel(item)}
          {/if}
        </span>
        <span class="batch-item-size">
          {#if item.size_after != null && item.state === "done"}
            {formatSize(item.size_before)} → {formatSize(item.size_after)}
          {:else}
            {formatSize(item.size_before)}
          {/if}
        </span>
      </li>
    {/each}
  </ul>

  {#if failures.length > 0}
    <details class="batch-failures">
      <summary>{t(locale, "batch.run.failures")} ({failures.length})</summary>
      <button type="button" class="ghost-btn" onclick={() => void copyFailures()}>
        {t(locale, failuresCopied ? "batch.run.failuresCopied" : "batch.run.copyFailures")}
      </button>
      <ul>
        {#each failures as item (item.id)}
          <li><strong>{item.file_name}</strong> — {item.error}</li>
        {/each}
      </ul>
    </details>
  {/if}
</div>
