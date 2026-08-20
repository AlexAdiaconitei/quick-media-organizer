export function isMacPlatform(): boolean {
  return (
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad/.test(navigator.platform)
  );
}

export function modLabel(letter: string): string {
  const key = letter.length === 1 ? letter.toUpperCase() : letter;
  return isMacPlatform() ? `⌘${key}` : `Ctrl+${key}`;
}

export function hasModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey;
}

export function modKey(event: KeyboardEvent): string | null {
  if (!hasModifier(event)) return null;
  return event.key.toLowerCase();
}

export function skipModLabel(): string {
  return isMacPlatform() ? "⌘⇧Space" : "Ctrl+Space";
}

export function isSkipShortcut(event: KeyboardEvent): boolean {
  if (event.key !== " " || event.altKey) return false;
  return isMacPlatform()
    ? event.metaKey && event.shiftKey && !event.ctrlKey
    : event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/// Platform-appropriate install command, so Windows users are not told to use
/// Homebrew.
export function ffmpegInstallCommand(): string {
  if (isMacPlatform()) return "brew install ffmpeg";
  if (typeof navigator !== "undefined" && /Win/i.test(navigator.platform)) {
    return "winget install Gyan.FFmpeg";
  }
  return "sudo apt install ffmpeg";
}
