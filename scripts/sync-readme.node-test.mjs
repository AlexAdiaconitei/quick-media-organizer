import assert from "node:assert/strict";
import test from "node:test";

import { pagesUrlFromSlug } from "./repo-slug.mjs";
import {
  isPerson,
  renderBadges,
  renderClone,
  renderContributors,
  replaceBlock,
} from "./sync-readme.mjs";

const copy = {
  docsBadge: "docs",
  docsAlt: "Documentation",
  releaseAlt: "Latest release",
  ciAlt: "CI",
  licenseAlt: "MIT License",
  platformAlt: "Platform",
  tauriAlt: "Tauri",
  clone: "Clone it:",
  none: "No contributors yet.",
  more: (count) => `…and ${count} more.`,
};

test("a project site lives under the repository name", () => {
  assert.equal(
    pagesUrlFromSlug("AlexAdiaconitei/quick-media-organizer"),
    "https://alexadiaconitei.github.io/quick-media-organizer/",
  );
});

test("a user site lives at the domain root", () => {
  assert.equal(
    pagesUrlFromSlug("Octocat/octocat.github.io"),
    "https://octocat.github.io/",
  );
});

test("badges name the current repository and link back relatively", () => {
  const badges = renderBadges("Someone/their-fork", "https://someone.github.io/their-fork/", copy);

  assert.match(badges, /github\/v\/release\/Someone\/their-fork/);
  assert.match(badges, /Someone\/their-fork\/actions\/workflows\/ci\.yml\/badge\.svg/);
  assert.match(badges, /\]\(\.\.\/\.\.\/releases\/latest\)/);
  assert.match(badges, /\]\(https:\/\/someone\.github\.io\/their-fork\/\)/);
  assert.doesNotMatch(badges, /AlexAdiaconitei|FerranVidalBelles/);
});

test("the clone command uses the current repository", () => {
  const clone = renderClone("Someone/their-fork", copy);

  assert.match(clone, /git clone https:\/\/github\.com\/Someone\/their-fork\.git/);
  assert.match(clone, /cd their-fork/);
});

test("bots are not listed as contributors", () => {
  assert.equal(isPerson({ login: "dependabot[bot]", type: "Bot" }), false);
  assert.equal(isPerson({ login: "renovate[bot]", type: "User" }), false);
  assert.equal(isPerson({ login: "octocat", type: "User" }), true);
});

test("the contributor block links every avatar to its profile", () => {
  const html = renderContributors(
    [
      { login: "octocat", type: "User", avatar_url: "https://avatars.example/1?v=4" },
      { login: "ci[bot]", type: "Bot", avatar_url: "https://avatars.example/2?v=4" },
    ],
    copy,
  );

  assert.match(html, /href="https:\/\/github\.com\/octocat"/);
  assert.match(html, /alt="octocat"/);
  assert.doesNotMatch(html, /ci\[bot\]/);
});

test("an empty contributor list falls back to a sentence", () => {
  assert.equal(renderContributors([], copy), "No contributors yet.");
});

test("a block is replaced in place and its markers survive", () => {
  const before = "intro\n<!-- badges:start -->\nold\n<!-- badges:end -->\noutro\n";
  const after = replaceBlock(before, "badges", "new");

  assert.equal(after, "intro\n<!-- badges:start -->\nnew\n<!-- badges:end -->\noutro\n");
});

test("a missing marker pair is an error, not a silent no-op", () => {
  assert.throws(() => replaceBlock("nothing here", "badges", "new"), /Missing/);
});
