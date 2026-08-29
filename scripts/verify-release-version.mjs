#!/usr/bin/env node

import { randomUUID } from "node:crypto";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
// pnpm forwards the "--" separator to the script, so drop it here.
export const cliArguments = (argv = process.argv.slice(2)) =>
  argv.filter((argument) => argument !== "--");
const SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

export function projectVersions(root = ROOT) {
  const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const tauriConfig = JSON.parse(
    readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"),
  );
  const cargoToml = readFileSync(
    join(root, "src-tauri", "Cargo.toml"),
    "utf8",
  );
  const cargoVersion = cargoToml.match(
    /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m,
  )?.[1];

  if (!cargoVersion) {
    throw new Error("Could not read the package version from src-tauri/Cargo.toml");
  }

  return {
    packageJson: packageJson.version,
    cargoToml: cargoVersion,
    tauriConfig: tauriConfig.version,
  };
}

export function normalizeReleaseVersion(input) {
  const value = String(input ?? "").trim().replace(/^v/i, "");
  const match = value.match(SEMVER);

  if (!match) {
    throw new Error(
      `Invalid release version "${input}". Use SemVer, for example 1.1.1 or 1.1.1-alpha.1.`,
    );
  }

  return {
    version: value,
    baseVersion: `${match[1]}.${match[2]}.${match[3]}`,
    hasPrereleaseSuffix: Boolean(match[4]),
  };
}

export function parsePrereleaseFlag(input) {
  if (input === true || input === false) return input;
  if (input === undefined || input === null || input === "") return false;
  if (String(input).toLowerCase() === "true") return true;
  if (String(input).toLowerCase() === "false") return false;
  throw new Error(`Invalid prerelease flag "${input}". Use true or false.`);
}

export function changelogNotes(changelog, baseVersion) {
  const lines = changelog.replace(/\r\n/g, "\n").split("\n");
  const headings = [];

  for (let index = 0; index < lines.length; index += 1) {
    const match = lines[index].match(
      /^##\s+\[?(\d+\.\d+\.\d+)\]?(?:\s+-\s+.+)?\s*$/,
    );
    if (match?.[1] === baseVersion) headings.push(index);
  }

  if (headings.length === 0) {
    throw new Error(`CHANGELOG.md has no section for ${baseVersion}.`);
  }
  if (headings.length > 1) {
    throw new Error(`CHANGELOG.md has more than one section for ${baseVersion}.`);
  }

  const start = headings[0] + 1;
  const relativeEnd = lines.slice(start).findIndex((line) => /^##\s+/.test(line));
  const end = relativeEnd === -1 ? lines.length : start + relativeEnd;
  const notes = lines.slice(start, end).join("\n").trim();

  if (!notes) {
    throw new Error(`The CHANGELOG.md section for ${baseVersion} is empty.`);
  }

  return notes;
}

export function releaseMetadata(
  requestedVersion,
  versions,
  changelog,
  prereleaseInput = false,
) {
  const normalized = normalizeReleaseVersion(requestedVersion);
  const uniqueVersions = new Set(Object.values(versions));

  if (uniqueVersions.size !== 1) {
    throw new Error(
      `Project versions do not match: ${Object.entries(versions)
        .map(([source, version]) => `${source}=${version}`)
        .join(", ")}`,
    );
  }
  if (versions.packageJson !== normalized.baseVersion) {
    throw new Error(
      `Release ${normalized.version} requires project version ${normalized.baseVersion}, but the project uses ${versions.packageJson}.`,
    );
  }

  const section = changelogNotes(changelog, normalized.baseVersion);
  const prerelease =
    normalized.hasPrereleaseSuffix || parsePrereleaseFlag(prereleaseInput);

  return {
    version: normalized.version,
    baseVersion: normalized.baseVersion,
    tag: `v${normalized.version}`,
    prerelease,
    releaseName: `Quick Media Organizer v${normalized.version}`,
    releaseNotes: `## Changes in ${normalized.baseVersion}\n\n${section}`,
  };
}

function writeGithubOutputs(path, metadata) {
  const delimiter = `release_notes_${randomUUID()}`;
  appendFileSync(
    path,
    [
      `version=${metadata.version}`,
      `base_version=${metadata.baseVersion}`,
      `tag=${metadata.tag}`,
      `prerelease=${metadata.prerelease}`,
      `release_name=${metadata.releaseName}`,
      `release_notes<<${delimiter}`,
      metadata.releaseNotes,
      delimiter,
      "",
    ].join("\n"),
  );
}

export function main(
  requestedVersion = cliArguments()[0],
  prereleaseInput = cliArguments()[1],
) {
  if (!requestedVersion) {
    throw new Error(
      "Pass a release version, for example: pnpm verify:release 1.1.1-alpha true",
    );
  }

  const changelog = readFileSync(join(ROOT, "CHANGELOG.md"), "utf8");
  const metadata = releaseMetadata(
    requestedVersion,
    projectVersions(),
    changelog,
    prereleaseInput,
  );

  if (process.env.RELEASE_CONFIG_PATH) {
    writeFileSync(
      resolve(process.env.RELEASE_CONFIG_PATH),
      `${JSON.stringify({ version: metadata.version }, null, 2)}\n`,
    );
  }
  if (process.env.RELEASE_NOTES_PATH) {
    writeFileSync(
      resolve(process.env.RELEASE_NOTES_PATH),
      `${metadata.releaseNotes}\n`,
    );
  }
  if (process.env.GITHUB_OUTPUT) {
    writeGithubOutputs(process.env.GITHUB_OUTPUT, metadata);
  }

  process.stdout.write(
    `Release ${metadata.tag} uses CHANGELOG.md section ${metadata.baseVersion} ` +
      `(prerelease: ${metadata.prerelease}).\n`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
