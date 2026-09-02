// One place for the values that change when the site moves or a fork builds it.
export const basePath = (process.env.NEXT_PUBLIC_BASE_PATH ?? '').replace(/\/$/, '');

/** Prefix a file in public/ with the deployment's base path. */
export function asset(path: string): string {
  return `${basePath}${path}`;
}

/**
 * owner/repo — also the key for the releases and contributors APIs.
 *
 * The workflow passes `${{ github.repository }}`, so a fork's site links to the
 * fork's own releases and lists the fork's own contributors. The fallback is
 * only for `next dev` on a clone.
 */
export const repoSlug =
  process.env.NEXT_PUBLIC_REPO_SLUG || 'AlexAdiaconitei/quick-media-organizer';

export const site = {
  name: 'Quick Media Organizer',
  tagline: 'Organize thousands of phone photos and videos with your keyboard.',
  repo: `https://github.com/${repoSlug}`,
  releases: `https://github.com/${repoSlug}/releases/latest`,
  releasesApi: `https://api.github.com/repos/${repoSlug}/releases/latest`,
  coffee: 'https://buymeacoffee.com/ferran_vidal',
};
