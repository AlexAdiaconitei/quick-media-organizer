import assert from "node:assert/strict";
import test from "node:test";

import {
  changelogNotes,
  cliArguments,
  normalizeReleaseVersion,
  releaseMetadata,
} from "./verify-release-version.mjs";

const matchingVersions = {
  packageJson: "1.1.1",
  cargoToml: "1.1.1",
  tauriConfig: "1.1.1",
};

const changelog = `# Changelog

## [Unreleased]

- Work in progress.

## [1.1.1] - 2026-08-29

### Added

- Manual releases.

### Fixed

- Installer startup.

## [1.1.0] - 2026-08-20

- Older change.
`;

test("a stable release uses its matching changelog section", () => {
  const metadata = releaseMetadata("v1.1.1", matchingVersions, changelog);

  assert.equal(metadata.version, "1.1.1");
  assert.equal(metadata.baseVersion, "1.1.1");
  assert.equal(metadata.tag, "v1.1.1");
  assert.equal(metadata.prerelease, false);
  assert.match(metadata.releaseNotes, /Manual releases/);
  assert.doesNotMatch(metadata.releaseNotes, /Older change/);
});

test("an alpha release includes the stable base version notes", () => {
  const metadata = releaseMetadata(
    "1.1.1-alpha.2",
    matchingVersions,
    changelog,
    false,
  );

  assert.equal(metadata.version, "1.1.1-alpha.2");
  assert.equal(metadata.baseVersion, "1.1.1");
  assert.equal(metadata.tag, "v1.1.1-alpha.2");
  assert.equal(metadata.prerelease, true);
  assert.match(metadata.releaseNotes, /^## Changes in 1\.1\.1/m);
  assert.match(metadata.releaseNotes, /Installer startup/);
});

test("the manual flag can mark a stable version as a prerelease", () => {
  const metadata = releaseMetadata(
    "1.1.1",
    matchingVersions,
    changelog,
    "true",
  );

  assert.equal(metadata.prerelease, true);
});

test("invalid semantic versions are rejected", () => {
  for (const version of ["alpha", "1.1", "1.1.1-01", "1.1.1 alpha"]) {
    assert.throws(() => normalizeReleaseVersion(version), /Invalid release version/);
  }
});

test("a release must match every project version", () => {
  assert.throws(
    () => releaseMetadata("1.1.2", matchingVersions, changelog),
    /requires project version 1\.1\.2/,
  );
  assert.throws(
    () =>
      releaseMetadata(
        "1.1.1",
        { ...matchingVersions, cargoToml: "1.1.0" },
        changelog,
      ),
    /Project versions do not match/,
  );
});

test("a release without changelog notes is rejected", () => {
  assert.throws(
    () => releaseMetadata("1.1.1", matchingVersions, "# Changelog\n"),
    /has no section for 1\.1\.1/,
  );
  assert.throws(
    () => changelogNotes("# Changelog\n\n## [1.1.1]\n", "1.1.1"),
    /section for 1\.1\.1 is empty/,
  );
});

test("the \"--\" separator pnpm forwards is ignored", () => {
  assert.deepEqual(cliArguments(["--", "1.1.1", "true"]), ["1.1.1", "true"]);
  assert.deepEqual(cliArguments(["1.1.1"]), ["1.1.1"]);
});
