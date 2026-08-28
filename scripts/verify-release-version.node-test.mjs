import assert from "node:assert/strict";
import test from "node:test";

import { verifyReleaseVersion } from "./verify-release-version.mjs";

const matchingVersions = {
  packageJson: "0.1.5",
  cargoToml: "0.1.5",
  tauriConfig: "0.1.5",
};

test("a release tag must match every project version", () => {
  assert.equal(verifyReleaseVersion("v0.1.5", matchingVersions), "0.1.5");
});

test("a mismatched tag is rejected", () => {
  assert.throws(
    () => verifyReleaseVersion("v0.1.6", matchingVersions),
    /does not match project version/,
  );
});

test("mismatched project files are rejected", () => {
  assert.throws(
    () =>
      verifyReleaseVersion("v0.1.5", {
        ...matchingVersions,
        cargoToml: "0.1.4",
      }),
    /Project versions do not match/,
  );
});
