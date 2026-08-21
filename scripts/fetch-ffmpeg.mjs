#!/usr/bin/env node
/**
 * Downloads ffmpeg + ffprobe for the current platform into src-tauri/binaries,
 * named the way Tauri's externalBin expects, so the installer can ship them and
 * the user does not have to install FFmpeg separately.
 *
 *   node scripts/fetch-ffmpeg.mjs
 *
 * Builds are pinned by version and checked against a known SHA-256. When the
 * pinned build disappears or the hash stops matching, update RELEASES below
 * rather than skipping the check.
 *
 * Licensing: these are GPL builds (they carry libx264/libx265). Shipping them
 * next to an MIT app is fine as an aggregate, but the GPL text and a pointer to
 * the corresponding sources must travel with the installer; the LICENSE file is
 * written next to the binaries and bundled as a resource.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { readdir, stat } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT_DIR = join(ROOT, "src-tauri", "binaries");
const TMP_DIR = join(ROOT, "src-tauri", ".ffmpeg-download");

/**
 * One entry per platform we ship installers for.
 * `sha256` is of the downloaded archive.
 */
const RELEASES = {
  "win32-x64": {
    target: "x86_64-pc-windows-msvc",
    exeSuffix: ".exe",
    licenseFile: "LICENSE",
    archives: [
      {
        url: "https://github.com/GyanD/codexffmpeg/releases/download/9.0/ffmpeg-9.0-essentials_build.zip",
        sha256: "e6b54767a6065919048f1a098eb27211ca4e12b4348a05d88777a5855d0b6e71",
      },
    ],
  },
  "darwin-arm64": {
    target: "aarch64-apple-darwin",
    exeSuffix: "",
    licenseFile: "",
    // ffmpeg publishes no official Apple Silicon binary; osxexperts.net is the
    // usual source. Pinned by hash because it is a third party.
    archives: [
      {
        url: "https://www.osxexperts.net/ffmpeg711arm.zip",
        sha256: "59e39a5cec2e5d2307ed079c53227a9181e64b87454ed4de998349e044bfdc70",
      },
      {
        url: "https://www.osxexperts.net/ffprobe711arm.zip",
        sha256: "e695da37c08c8fbc218ebc161ee20d5606b50f3c7e8d696cbcf01bd40fe20d7e",
      },
    ],
  },
  "linux-x64": {
    target: "x86_64-unknown-linux-gnu",
    exeSuffix: "",
    licenseFile: "LICENSE.txt",
    // A rolling "latest" build: the hash changes whenever BtbN rebuilds, so it
    // is re-pinned rather than trusted blindly.
    archives: [
      {
        url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz",
        sha256: "5eed1ef9625abbcfeaeb8f7af137b9d0212a5c554624d4780f2fc2e344b64a26",
      },
    ],
  },
};

const FFMPEG_SOURCE_NOTICE = `FFmpeg is distributed with this application under the GNU General Public
License v3. It is a separate program, invoked as a subprocess; the application
itself is MIT licensed.

Bundled build: {url}

The corresponding source code for this FFmpeg build is available from
https://ffmpeg.org/download.html and from the build provider linked above.
`;

function platformKey() {
  const key = `${process.platform}-${process.arch}`;
  if (!RELEASES[key]) {
    throw new Error(
      `No pinned FFmpeg build for ${key}. Add one to scripts/fetch-ffmpeg.mjs.`,
    );
  }
  return key;
}

async function download(url, destination) {
  process.stdout.write(`  downloading ${url}\n`);
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText} for ${url}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  writeFileSync(destination, bytes);
  return createHash("sha256").update(bytes).digest("hex");
}

function extract(archive, into) {
  // bsdtar ships with Windows 10+, macOS and most Linux images, and reads both
  // zip and tar.xz. On Windows the System32 one must be named explicitly: a
  // GNU tar from Git Bash may come first on PATH and reads "D:\..." as a
  // remote host.
  const tarBin =
    process.platform === "win32"
      ? join(process.env.SystemRoot ?? "C:\Windows", "System32", "tar.exe")
      : "tar";
  execFileSync(tarBin, ["-xf", archive, "-C", into], { stdio: "inherit" });
}

async function findFile(dir, name) {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const entry of entries) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      const found = await findFile(full, name);
      if (found) return found;
    } else if (entry.name === name) {
      return full;
    }
  }
  return null;
}

async function main() {
  const key = platformKey();
  const release = RELEASES[key];

  mkdirSync(OUT_DIR, { recursive: true });
  rmSync(TMP_DIR, { recursive: true, force: true });
  mkdirSync(TMP_DIR, { recursive: true });

  for (const [index, source] of release.archives.entries()) {
    const extension = source.url.endsWith(".tar.xz") ? ".tar.xz" : ".zip";
    const archive = join(TMP_DIR, `archive-${index}${extension}`);
    const digest = await download(source.url, archive);

    if (source.sha256 && digest !== source.sha256) {
      throw new Error(
        [
          `Checksum mismatch for ${source.url}`,
          `  expected ${source.sha256}`,
          `  got      ${digest}`,
        ].join("\n"),
      );
    }
    if (!source.sha256) {
      process.stdout.write(
        `  WARNING: no pinned checksum. Downloaded sha256 is ${digest}\n`,
      );
    }
    extract(archive, TMP_DIR);
  }

  for (const name of ["ffmpeg", "ffprobe"]) {
    const fileName = `${name}${release.exeSuffix}`;
    const found = await findFile(TMP_DIR, fileName);
    if (!found) throw new Error(`${fileName} not found in the archive`);
    const target = join(OUT_DIR, `${name}-${release.target}${release.exeSuffix}`);
    rmSync(target, { force: true });
    renameSync(found, target);
    if (process.platform !== "win32") {
      // Zip archives do not always carry the executable bit.
      chmodSync(target, 0o755);
    }
    const size = (await stat(target)).size / 1024 / 1024;
    process.stdout.write(`  ${target} (${size.toFixed(1)} MB)\n`);
  }

  const licensePath = release.licenseFile ? await findFile(TMP_DIR, release.licenseFile) : null;
  const notice = FFMPEG_SOURCE_NOTICE.replace(
    "{url}",
    release.archives.map((source) => source.url).join(", "),
  );
  writeFileSync(
    join(OUT_DIR, "FFMPEG-LICENSE.txt"),
    licensePath ? `${notice}

${readFileSync(licensePath, "utf8")}` : notice,
  );

  rmSync(TMP_DIR, { recursive: true, force: true });
  process.stdout.write("FFmpeg is ready to be bundled.\n");
}

main().catch((error) => {
  rmSync(TMP_DIR, { recursive: true, force: true });
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
});

export { RELEASES, platformKey };
