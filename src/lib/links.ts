export const LINKS = {
  buyMeACoffee: "https://buymeacoffee.com/ferran_vidal",
  email: "mailto:ferranvidaldev@gmail.com",
  linkedIn: "https://www.linkedin.com/in/ferran-vidal-belles/",
} as const;

export async function openExternal(url: string): Promise<void> {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
