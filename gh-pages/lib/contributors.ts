import { repoSlug } from '@/lib/site';

/**
 * Read once while `next build` runs, then baked into the exported HTML. The
 * page therefore needs no JavaScript and no API call from the visitor, and the
 * list refreshes whenever the site is rebuilt (the workflow also runs weekly).
 */

export interface Contributor {
  login: string;
  avatarUrl: string;
  profileUrl: string;
  contributions: number;
}

interface ApiContributor {
  login: string;
  type: string;
  avatar_url: string;
  html_url: string;
  contributions: number;
}

const isPerson = (c: ApiContributor) => c.type !== 'Bot' && !/\[bot\]$/i.test(c.login);

export async function getContributors(): Promise<Contributor[]> {
  const token = process.env.GITHUB_TOKEN;

  try {
    const response = await fetch(
      `https://api.github.com/repos/${repoSlug}/contributors?per_page=100`,
      {
        headers: {
          Accept: 'application/vnd.github+json',
          ...(token ? { Authorization: `Bearer ${token}` } : {}),
        },
        signal: AbortSignal.timeout(10_000),
      },
    );

    if (!response.ok) throw new Error(`GitHub answered ${response.status}`);

    const people = (await response.json()) as ApiContributor[];
    return people.filter(isPerson).map((person) => ({
      login: person.login,
      avatarUrl: `${person.avatar_url}&s=160`,
      profileUrl: person.html_url,
      contributions: person.contributions,
    }));
  } catch (error) {
    // A rate limit or an offline build must not fail the site. The section
    // renders its fallback and links to the graph on GitHub instead.
    console.warn(`Could not read contributors for ${repoSlug}:`, error);
    return [];
  }
}
