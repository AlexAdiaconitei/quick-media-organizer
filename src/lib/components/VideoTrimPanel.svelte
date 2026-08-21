<script lang="ts">
  import { format, t, type Locale } from "../i18n";
  import { ffmpegInstallCommand, modLabel } from "../shortcuts";

  let {
    locale,
    videoRef = $bindable<HTMLVideoElement | null>(null),
    pendingTrim = $bindable(false),
    ffmpegAvailable = true,
    disabled = false,
    screenshotDemo = false,
    onApply,
  }: {
    locale: Locale;
    videoRef?: HTMLVideoElement | null;
    pendingTrim?: boolean;
    ffmpegAvailable?: boolean;
    disabled?: boolean;
    screenshotDemo?: boolean;
    onApply: (trimStart: number, trimEnd: number) => void;
  } = $props();

  let duration = $state(0);
  let trimStart = $state(0);
  let trimEnd = $state(0);
  let lastVideoSrc = $state("");

  const canApply = $derived(
    ffmpegAvailable &&
      !disabled &&
      duration > 0 &&
      trimEnd > trimStart + 0.05 &&
      (trimStart > 0.05 || duration - trimEnd > 0.05),
  );

  const startPct = $derived(duration > 0 ? (trimStart / duration) * 100 : 0);
  const keepPct = $derived(duration > 0 ? ((trimEnd - trimStart) / duration) * 100 : 100);

  $effect(() => {
    pendingTrim = canApply;
  });

  export function getTrimRange(): { start: number; end: number } | null {
    if (!canApply) return null;
    return { start: trimStart, end: trimEnd };
  }

  export function resetAfterApply() {
    resetTrim();
  }

  function formatTime(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
    const total = Math.floor(seconds);
    const mins = Math.floor(total / 60);
    const secs = total % 60;
    const ms = Math.floor((seconds - total) * 10);
    return `${mins}:${secs.toString().padStart(2, "0")}.${ms}`;
  }

  function syncFromVideo() {
    if (!videoRef) return;
    const nextDuration = videoRef.duration;
    if (!Number.isFinite(nextDuration) || nextDuration <= 0) return;

    const src = videoRef.currentSrc || videoRef.src;
    if (src !== lastVideoSrc) {
      lastVideoSrc = src;
      duration = nextDuration;
      if (screenshotDemo) {
        trimStart = nextDuration * 0.12;
        trimEnd = nextDuration * 0.78;
        videoRef.currentTime = trimStart + (trimEnd - trimStart) * 0.35;
      } else {
        trimStart = 0;
        trimEnd = nextDuration;
        videoRef.currentTime = 0;
      }
      return;
    }

    duration = nextDuration;
    trimEnd = Math.min(trimEnd, nextDuration);
    trimStart = Math.min(trimStart, trimEnd - 0.05);
  }

  function clampRange() {
    if (duration <= 0) return;
    trimStart = Math.max(0, Math.min(trimStart, duration - 0.05));
    trimEnd = Math.max(trimStart + 0.05, Math.min(trimEnd, duration));
  }

  export function setStartToPlayhead() {
    if (!videoRef) return;
    trimStart = Math.min(videoRef.currentTime, trimEnd - 0.05);
    clampRange();
    videoRef.currentTime = trimStart;
  }

  export function setEndToPlayhead() {
    if (!videoRef) return;
    trimEnd = Math.max(videoRef.currentTime, trimStart + 0.05);
    clampRange();
    videoRef.currentTime = trimEnd;
  }

  export function resetTrim() {
    if (!videoRef || duration <= 0) return;
    trimStart = 0;
    trimEnd = duration;
    videoRef.currentTime = 0;
  }

  export function applyTrim() {
    if (!canApply) return;
    onApply(trimStart, trimEnd);
  }

  function handleStartInput(event: Event) {
    trimStart = Number((event.target as HTMLInputElement).value);
    clampRange();
    if (videoRef) videoRef.currentTime = trimStart;
  }

  function handleEndInput(event: Event) {
    trimEnd = Number((event.target as HTMLInputElement).value);
    clampRange();
    if (videoRef) videoRef.currentTime = trimEnd;
  }

  function blurTrimHandles() {
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
  }

  $effect(() => {
    const video = videoRef;
    if (!video) return;

    const onMeta = () => syncFromVideo();
    const onTimeUpdate = () => {
      if (!videoRef || duration <= 0) return;
      if (videoRef.currentTime < trimStart - 0.02) {
        videoRef.currentTime = trimStart;
      }
      if (videoRef.currentTime > trimEnd + 0.02) {
        videoRef.pause();
        videoRef.currentTime = trimEnd;
      }
    };

    video.addEventListener("loadedmetadata", onMeta);
    video.addEventListener("durationchange", onMeta);
    video.addEventListener("timeupdate", onTimeUpdate);
    syncFromVideo();

    return () => {
      video.removeEventListener("loadedmetadata", onMeta);
      video.removeEventListener("durationchange", onMeta);
      video.removeEventListener("timeupdate", onTimeUpdate);
    };
  });
</script>

<div class="video-trim-panel">
  <div class="trim-header">
    <strong>{t(locale, "trim.title")}</strong>
    <span class="lossless-badge">{t(locale, "trim.lossless")}</span>
  </div>

  {#if !ffmpegAvailable}
    <p class="trim-warning">
      {format(locale, "trim.ffmpegMissing", { command: ffmpegInstallCommand() })}
    </p>
  {/if}

  <div class="trim-range" aria-label={t(locale, "trim.title")}>
    <div class="trim-range-track" aria-hidden="true">
      <div class="trim-cut trim-cut-left" style={`width: ${startPct}%`}></div>
      <div class="trim-keep" style={`left: ${startPct}%; width: ${keepPct}%`}></div>
      <div
        class="trim-cut trim-cut-right"
        style={`left: ${startPct + keepPct}%; width: ${100 - startPct - keepPct}%`}
      ></div>
    </div>

    <input
      class="trim-handle trim-handle-start"
      type="range"
      min="0"
      max={duration || 1}
      step="0.05"
      value={trimStart}
      disabled={disabled || duration <= 0}
      aria-label={t(locale, "trim.start")}
      oninput={handleStartInput}
      onchange={blurTrimHandles}
      onpointerup={blurTrimHandles}
    />
    <input
      class="trim-handle trim-handle-end"
      type="range"
      min="0"
      max={duration || 1}
      step="0.05"
      value={trimEnd}
      disabled={disabled || duration <= 0}
      aria-label={t(locale, "trim.end")}
      oninput={handleEndInput}
      onchange={blurTrimHandles}
      onpointerup={blurTrimHandles}
    />
  </div>

  <div class="trim-range-labels">
    <span>{formatTime(trimStart)}</span>
    <span class="trim-range-kept">
      {format(locale, "trim.kept", { duration: formatTime(trimEnd - trimStart) })}
    </span>
    <span>{formatTime(trimEnd)}</span>
  </div>

  <div class="trim-actions">
    <button type="button" class="ghost-btn" disabled={disabled} onclick={setStartToPlayhead}>
      {t(locale, "trim.setStart")}
    </button>
    <button type="button" class="ghost-btn" disabled={disabled} onclick={setEndToPlayhead}>
      {t(locale, "trim.setEnd")}
    </button>
    <button type="button" class="ghost-btn" disabled={disabled} onclick={resetTrim}>
      {t(locale, "trim.reset")}
    </button>
    <button type="button" class="primary-btn" disabled={!canApply} onclick={applyTrim}>
      {t(locale, "trim.apply")}
    </button>
  </div>

  <p class="trim-hint">{format(locale, "trim.hint", { startKey: "[", endKey: "]" })}</p>
  {#if pendingTrim}
    <p class="trim-enter-note">{t(locale, "trim.enterSavesToo")}</p>
  {/if}
  <p class="trim-note">{t(locale, "trim.keyframeNote")}</p>
  <p class="trim-note trim-destructive-note">
    {format(locale, "trim.replacesOriginal", {
      folder: ".quick-media-organizer/trim-backups/",
      key: modLabel("Z"),
    })}
  </p>
</div>
