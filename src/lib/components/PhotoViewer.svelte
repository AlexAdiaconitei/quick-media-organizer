<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { openPath } from "@tauri-apps/plugin-opener";
  import { invokeLogged, reportError } from "../errorReporter";
  import { t, type Locale } from "../i18n";
  import type { MediaFileDiagnosis, MediaItem, VideoPreviewInfo } from "../types";

  let {
    locale,
    item,
    videoRef = $bindable<HTMLVideoElement | null>(null),
    demoMode = false,
    videoWithSound = false,
    onError = () => {},
  }: {
    locale: Locale;
    item: MediaItem | null | undefined;
    videoRef?: HTMLVideoElement | null;
    demoMode?: boolean;
    videoWithSound?: boolean;
    onError?: (message: string) => void;
  } = $props();

  const VIDEO_VOLUME = 1;

  let previewInfo = $state<VideoPreviewInfo | null>(null);
  let previewLoading = $state(false);
  let playbackFailed = $state(false);
  let imageFailed = $state(false);
  let imageIssue = $state<MediaFileDiagnosis["issue"] | null>(null);

  function applyVideoAudio(video: HTMLVideoElement | null) {
    if (!video) return;
    video.muted = !videoWithSound;
    video.volume = videoWithSound ? VIDEO_VOLUME : 1;
  }

  $effect(() => {
    applyVideoAudio(videoRef);
  });

  const mediaKey = $derived(`${item?.paths[0] ?? ""}|${item?.size_bytes ?? 0}`);

  $effect(() => {
    const path = item?.paths[0];
    const isVideo = item?.is_video;
    // Re-resolve whenever the bytes on disk change, otherwise a trimmed video
    // keeps playing the stale proxy built before the cut.
    void mediaKey;

    playbackFailed = false;
    previewInfo = null;
    imageFailed = false;
    imageIssue = null;

    if (!isVideo || !path || demoMode) {
      previewLoading = false;
      return;
    }

    let cancelled = false;
    previewLoading = true;

    void invokeLogged<VideoPreviewInfo>("resolve_video_preview", { path })
      .then((info) => {
        if (!cancelled) previewInfo = info;
      })
      .finally(() => {
        if (!cancelled) previewLoading = false;
      });

    return () => {
      cancelled = true;
    };
  });

  const previewPath = $derived(item?.paths[0] ?? "");
  const playbackPath = $derived(
    demoMode ? previewPath : (previewInfo?.playback_path ?? previewPath),
  );
  const showFallback = $derived(
    !demoMode &&
      !!item?.is_video &&
      !previewLoading &&
      (previewInfo?.mode === "unavailable" || playbackFailed),
  );
  const assetUrl = $derived(
    demoMode && previewPath
      ? previewPath
      : playbackPath
        ? `${convertFileSrc(playbackPath)}?v=${encodeURIComponent(mediaKey)}`
        : "",
  );
  const posterUrl = $derived(
    previewInfo?.poster_path
      ? `${convertFileSrc(previewInfo.poster_path)}?v=${encodeURIComponent(mediaKey)}`
      : "",
  );

  async function openInDefaultApp() {
    if (!previewPath) return;
    try {
      await openPath(previewPath);
    } catch (error) {
      // Silently swallowing this is what made the button look dead.
      onError(String(error));
      void reportError(String(error), { action: "open_in_default_app", path: previewPath });
    }
  }

  function imageIssueMessage(issue: MediaFileDiagnosis["issue"] | null): string {
    switch (issue) {
      case "empty":
        return t(locale, "preview.imageEmpty");
      case "too_small":
        return t(locale, "preview.imageTooSmall");
      case "content_mismatch":
        return t(locale, "preview.imageContentMismatch");
      case "unknown":
        return t(locale, "preview.imageUnknown");
      default:
        return t(locale, "preview.imageFailed");
    }
  }

  async function handleImageError() {
    imageFailed = true;
    if (!previewPath || demoMode) return;
    try {
      const diagnosis = await invokeLogged<MediaFileDiagnosis>("diagnose_media_file", {
        path: previewPath,
      });
      imageIssue = diagnosis.issue;
    } catch {
      imageIssue = "unknown";
    }
  }

  function handleVideoError() {
    playbackFailed = true;
  }
</script>

<div class="preview-panel">
  {#if item}
    {#key mediaKey}
      {#if item.kind === "live_photo"}
        <span class="live-badge">{t(locale, "livePhoto")}</span>
      {/if}

      <div class="preview-stage">
        {#if item.is_video}
          {#if previewLoading}
            <div class="video-preview-status">{t(locale, "preview.preparing")}</div>
          {:else if showFallback}
            <div class="video-preview-fallback">
              {#if posterUrl}
                <img class="video-preview-poster" src={posterUrl} alt={item.file_name} />
              {/if}
              <div class="video-preview-copy">
                <p>{t(locale, "preview.unsupportedFormat")}</p>
                <button type="button" class="ghost-btn" onclick={() => void openInDefaultApp()}>
                  {t(locale, "preview.openExternal")}
                </button>
              </div>
            </div>
          {:else if assetUrl}
            <video
              bind:this={videoRef}
              class="preview-media"
              src={assetUrl}
              controls
              autoplay
              muted={!videoWithSound}
              playsinline
              onloadeddata={(event) => applyVideoAudio(event.currentTarget)}
              onerror={handleVideoError}
            ></video>
          {/if}
          {#if previewInfo?.mode === "proxy" && !showFallback && !previewLoading}
            <p class="video-preview-hint">{t(locale, "preview.proxyHint")}</p>
          {/if}
        {:else if imageFailed}
          <div class="video-preview-fallback">
            <div class="video-preview-copy">
              <p>{imageIssueMessage(imageIssue)}</p>
              <p class="preview-file-name">{item.file_name}</p>
              <button type="button" class="ghost-btn" onclick={() => void openInDefaultApp()}>
                {t(locale, "preview.openExternal")}
              </button>
            </div>
          </div>
        {:else}
          <img
            class="preview-media"
            src={assetUrl}
            alt={item.file_name}
            onerror={() => void handleImageError()}
          />
        {/if}
      </div>
    {/key}
  {/if}
</div>
