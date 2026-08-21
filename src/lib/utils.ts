export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatDate(value?: string | null): string {
  if (!value) return "—";
  return value
    // EXIF dates come as 2024:08:12 19:42:00
    .replace(/^(\d{4}):(\d{2}):(\d{2})/, "$1-$2-$3")
    // ISO timestamps read better without the T and the seconds
    .replace("T", " ")
    .replace(/(\d{2}:\d{2}):\d{2}(\.\d+)?Z?$/, "$1");
}
