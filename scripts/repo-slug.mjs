/**
 * Works out which GitHub repository is being built, so nothing in the project
 * has to name it. GITHUB_REPOSITORY is set inside Actions; a local run falls
 * back to the "origin" remote. A fork therefore points at itself everywhere,
 * with no URL to edit after cloning.
 */

import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export function repositorySlugFromRemote(remote) {
  const match = remote.match(/github\.com[:/]([^/]+)\/([^/]+?)(\.git)?$/);
  if (!match) {
    throw new Error(`Cannot work out the GitHub repository from "${remote}"`);
  }
  return `${match[1]}/${match[2]}`;
}

export function repositorySlug() {
  if (process.env.GITHUB_REPOSITORY) return process.env.GITHUB_REPOSITORY;

  const remote = execFileSync("git", ["remote", "get-url", "origin"], {
    cwd: ROOT,
    encoding: "utf8",
  }).trim();

  return repositorySlugFromRemote(remote);
}

/**
 * The GitHub Pages address for a slug. A repository named
 * "<owner>.github.io" is a user site and lives at the domain root; everything
 * else is a project site under /<repo>/.
 *
 * A custom domain configured through a CNAME file is not visible from here, so
 * a repository using one should pass its own URL instead.
 */
export function pagesUrlFromSlug(slug) {
  const [owner, repo] = slug.split("/");
  if (!owner || !repo) {
    throw new Error(`Expected an "owner/repo" slug, got "${slug}"`);
  }

  const host = `${owner.toLowerCase()}.github.io`;
  return repo.toLowerCase() === host ? `https://${host}/` : `https://${host}/${repo}/`;
}
