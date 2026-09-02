'use client';

import Link from 'next/link';
import { useEffect, useState } from 'react';
import { site } from '@/lib/site';

/**
 * Installer filenames carry the version, so there is no stable
 * releases/latest/download/<name> URL to hardcode. The buttons start out
 * pointing at the releases page and are upgraded to the exact asset once the
 * releases API answers. If it never answers, the releases page is still the
 * right destination.
 */

interface Asset {
  name: string;
  browser_download_url: string;
  size: number;
}

interface Build {
  url: string;
  detail: string;
}

interface Latest {
  tag: string;
  windows: Build | null;
  macos: Build | null;
}

const CACHE_KEY = 'qmo:latest-release';
const CACHE_TTL = 6 * 60 * 60 * 1000;

const megabytes = (bytes: number) => `${(bytes / 1_000_000).toFixed(0)} MB`;

// The Lite installers ship in the same release; the buttons offer the Standard
// build, which needs no separate FFmpeg install.
const standard = (asset: Asset) => !/-lite\./i.test(asset.name);

function pick(assets: Asset[], ...patterns: RegExp[]): Asset | undefined {
  for (const pattern of patterns) {
    const found = assets.find((a) => standard(a) && pattern.test(a.name));
    if (found) return found;
  }
  return undefined;
}

function build(asset: Asset | undefined, kind: string): Build | null {
  if (!asset) return null;
  return {
    url: asset.browser_download_url,
    detail: `${kind} · ${megabytes(asset.size)}`,
  };
}

function readCache(): Latest | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;
    const cached = JSON.parse(raw) as { at: number; latest: Latest };
    if (Date.now() - cached.at > CACHE_TTL) return null;
    return cached.latest;
  } catch {
    return null;
  }
}

export function DownloadButtons({
  /** The hero adds the release note and a docs link; later sections have both already. */
  variant = 'hero',
}: {
  variant?: 'hero' | 'plain';
} = {}) {
  const [latest, setLatest] = useState<Latest | null>(null);
  const [isMac, setIsMac] = useState(false);

  useEffect(() => {
    const platform =
      (navigator as { userAgentData?: { platform?: string } }).userAgentData?.platform ??
      navigator.userAgent;
    setIsMac(/mac|iphone|ipad/i.test(platform));

    const cached = readCache();
    if (cached) {
      setLatest(cached);
      return;
    }

    const abort = new AbortController();
    fetch(site.releasesApi, {
      signal: abort.signal,
      headers: { Accept: 'application/vnd.github+json' },
    })
      .then((response) => (response.ok ? response.json() : Promise.reject(response.status)))
      .then((release: { tag_name: string; assets: Asset[] }) => {
        const assets = release.assets ?? [];
        const resolved: Latest = {
          tag: release.tag_name,
          windows: build(pick(assets, /-setup\.exe$/i, /\.exe$/i, /\.msi$/i), 'Windows installer'),
          macos: build(pick(assets, /\.dmg$/i), 'Apple Silicon'),
        };
        setLatest(resolved);
        try {
          localStorage.setItem(CACHE_KEY, JSON.stringify({ at: Date.now(), latest: resolved }));
        } catch {
          // A browser with site data blocked just refetches next visit.
        }
      })
      .catch(() => {
        // Buttons keep their releases-page href.
      });

    return () => abort.abort();
  }, []);

  const windows = {
    label: 'Download for Windows',
    href: latest?.windows?.url ?? site.releases,
    detail: latest?.windows?.detail ?? 'Windows installer',
  };
  const macos = {
    label: 'Download for macOS',
    href: latest?.macos?.url ?? site.releases,
    detail: latest?.macos?.detail ?? 'Apple Silicon',
  };

  // The visitor's own platform leads.
  const [first, second] = isMac ? [macos, windows] : [windows, macos];

  return (
    <div className="lp-downloads">
      <div className="lp-cta">
        <a className="lp-btn lp-btn-primary lp-btn-dl" href={first.href}>
          <span>{first.label}</span>
          <small>{first.detail}</small>
        </a>
        <a className="lp-btn lp-btn-ghost lp-btn-dl" href={second.href}>
          <span>{second.label}</span>
          <small>{second.detail}</small>
        </a>
        <a className="lp-btn lp-btn-quiet" href={site.releases}>
          All releases
        </a>
        {variant === 'hero' ? (
          <Link className="lp-btn lp-btn-quiet" href="/docs">
            Read the docs
          </Link>
        ) : null}
      </div>

      {variant === 'hero' ? (
        <p className="lp-release-note">
          {latest?.tag ? `${latest.tag} · ` : null}
          Free and MIT licensed. Lite builds, checksums and older versions are on
          the releases page.
        </p>
      ) : null}
    </div>
  );
}
