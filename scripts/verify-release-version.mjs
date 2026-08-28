#!/usr/bin/env node

import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

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

export function verifyReleaseVersion(tag, versions) {
  const uniqueVersions = new Set(Object.values(versions));
  if (uniqueVersions.size !== 1) {
    throw new Error(
      `Project versions do not match: ${Object.entries(versions)
        .map(([source, version]) => `${source}=${version}`)
        .join(", ")}`,
    );
  }

  const version = versions.packageJson;
  if (tag !== `v${version}`) {
    throw new Error(`Release tag ${tag} does not match project version v${version}`);
  }

  return version;
}

export function main(tag = process.argv[2]) {
  if (!tag) {
    throw new Error("Pass the release tag, for example: pnpm verify:release -- v0.1.5");
  }

  const version = verifyReleaseVersion(tag, projectVersions());
  process.stdout.write(`Release tag and project files agree on version ${version}.\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
