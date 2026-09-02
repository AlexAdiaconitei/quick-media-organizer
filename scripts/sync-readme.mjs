#!/usr/bin/env node
/**
 * Rewrites the parts of the READMEs that depend on which repository they live
 * in: the badge row, the clone command, and the contributor list.
 *
 *   node scripts/sync-readme.mjs
 *
 * Everything else in those files is written by hand and never touched here.
 * Links that GitHub can resolve relatively (../../releases, ../../issues) are
 * left as they are; badge images and the Pages address cannot be relative, so
 * they are generated from the current repository instead of being typed in.
 *
 * Run by .github/workflows/readme.yml, which commits any change. A fork
 * rewrites these blocks to point at itself on its first push to main.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { pagesUrlFromSlug, repositorySlug } from "./repo-slug.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Avatars shown before the list is cut short with a link to the full graph. */
const MAX_AVATARS = 24;
const AVATAR_SIZE = 56;

const COPY = {
  "README.md": {
    docsBadge: "docs",
    docsAlt: "Documentation",
    releaseAlt: "Latest release",
    ciAlt: "CI",
    licenseAlt: "MIT License",
    platformAlt: "Platform",
    tauriAlt: "Tauri",
    clone: "Clone and run it locally:",
    none: "Contributor avatars appear here once the repository has some.",
    more: (count) => `…and ${count} more on the [contributor graph](../../graphs/contributors).`,
  },
  "README.es.md": {
    docsBadge: "docs",
    docsAlt: "Documentación",
    releaseAlt: "Última versión",
    ciAlt: "CI",
    licenseAlt: "Licencia MIT",
    platformAlt: "Plataformas",
    tauriAlt: "Tauri",
    clone: "Clónalo y ejecútalo en local:",
    none: "Los avatares aparecerán aquí en cuanto el repositorio tenga colaboradores.",
    more: (count) =>
      `…y ${count} más en el [grafo de colaboradores](../../graphs/contributors).`,
  },
};

export function renderBadges(slug, pagesUrl, copy) {
  const encodedSlug = slug.split("/").map(encodeURIComponent).join("/");

  return [
    `[![${copy.docsAlt}](https://img.shields.io/badge/${copy.docsBadge}-website-d4cfc7?style=flat&labelColor=101014)](${pagesUrl})`,
    `[![${copy.releaseAlt}](https://img.shields.io/github/v/release/${encodedSlug}?style=flat&labelColor=101014&color=d4cfc7)](../../releases/latest)`,
    `[![${copy.ciAlt}](https://github.com/${encodedSlug}/actions/workflows/ci.yml/badge.svg)](../../actions/workflows/ci.yml)`,
    `![${copy.licenseAlt}](https://img.shields.io/badge/license-MIT-blue)`,
    `![${copy.platformAlt}](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey)`,
    `![${copy.tauriAlt}](https://img.shields.io/badge/built%20with-Tauri%202-orange)`,
  ].join("\n");
}

export function renderClone(slug, copy) {
  const repo = slug.split("/")[1];

  return [
    copy.clone,
    "",
    "```bash",
    `git clone https://github.com/${slug}.git`,
    `cd ${repo}`,
    "pnpm install",
    "pnpm dev",
    "```",
  ].join("\n");
}

/** GitHub's own automation should not be listed as a person. */
export function isPerson(contributor) {
  return contributor.type !== "Bot" && !/\[bot\]$/i.test(contributor.login);
}

export function renderContributors(contributors, copy) {
  const people = contributors.filter(isPerson);
  if (people.length === 0) return copy.none;

  const shown = people.slice(0, MAX_AVATARS);
  const avatars = shown
    .map(
      (person) =>
        `<a href="https://github.com/${person.login}" title="${person.login}">` +
        `<img src="${person.avatar_url}&s=${AVATAR_SIZE * 2}" width="${AVATAR_SIZE}" ` +
        `height="${AVATAR_SIZE}" alt="${person.login}" /></a>`,
    )
    .join("\n");

  const remaining = people.length - shown.length;
  return remaining > 0 ? `${avatars}\n\n${copy.more(remaining)}` : avatars;
}

export function replaceBlock(text, name, body) {
  const start = `<!-- ${name}:start -->`;
  const end = `<!-- ${name}:end -->`;
  const pattern = new RegExp(`${start}[\\s\\S]*?${end}`);

  if (!pattern.test(text)) {
    throw new Error(`Missing ${start} … ${end} markers`);
  }

  return text.replace(pattern, `${start}\n${body}\n${end}`);
}

async function fetchContributors(slug) {
  const token = process.env.GITHUB_TOKEN;
  const response = await fetch(
    `https://api.github.com/repos/${slug}/contributors?per_page=100`,
    {
      headers: {
        Accept: "application/vnd.github+json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
    },
  );

  if (!response.ok) {
    throw new Error(`GitHub answered ${response.status} for ${slug} contributors`);
  }

  return response.json();
}

export async function main() {
  const slug = repositorySlug();
  const pagesUrl = process.env.PAGES_URL?.trim() || pagesUrlFromSlug(slug);

  let contributors = null;
  try {
    contributors = await fetchContributors(slug);
  } catch (error) {
    // A rate limit or an offline run should not rewrite a good list with an
    // empty one, so the existing block is left exactly as it is.
    process.stdout.write(`Skipping the contributor list: ${error.message}\n`);
  }

  for (const [file, copy] of Object.entries(COPY)) {
    const path = join(ROOT, file);
    const original = readFileSync(path, "utf8");

    let updated = replaceBlock(original, "badges", renderBadges(slug, pagesUrl, copy));
    updated = replaceBlock(updated, "clone", renderClone(slug, copy));
    if (contributors) {
      updated = replaceBlock(
        updated,
        "contributors",
        renderContributors(contributors, copy),
      );
    }

    if (updated !== original) {
      writeFileSync(path, updated);
      process.stdout.write(`Updated ${file}\n`);
    }
  }

  process.stdout.write(`Repository ${slug}, site ${pagesUrl}\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await main();
}
