import assert from "node:assert/strict";
import test from "node:test";

import {
  repositorySlugFromRemote,
  updaterSigningMode,
} from "./updater-config.mjs";

test("the updater is disabled when both signing keys are absent", () => {
  assert.equal(updaterSigningMode(undefined, undefined), "disabled");
});

test("the updater rejects a partial signing configuration", () => {
  assert.throws(
    () => updaterSigningMode("public", undefined),
    /must either both be set or both be absent/,
  );
  assert.throws(
    () => updaterSigningMode(undefined, "private"),
    /must either both be set or both be absent/,
  );
});

test("the updater is enabled when both signing keys exist", () => {
  assert.equal(updaterSigningMode("public", "private"), "enabled");
});

test("GitHub HTTPS and SSH remotes produce the same repository slug", () => {
  assert.equal(
    repositorySlugFromRemote(
      "https://github.com/AlexAdiaconitei/quick-media-organizer.git",
    ),
    "AlexAdiaconitei/quick-media-organizer",
  );
  assert.equal(
    repositorySlugFromRemote(
      "git@github.com:AlexAdiaconitei/quick-media-organizer.git",
    ),
    "AlexAdiaconitei/quick-media-organizer",
  );
});
