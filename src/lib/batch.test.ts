import { describe, expect, it } from "vitest";

import {
  applyRescan,
  formatFailureReport,
  itemIsInsideFolder,
  metadataFate,
  shouldReseedQueue,
  sanitizeStoredSettings,
  shouldFinalizeRecoveredJob,
} from "./batch";
import type { BatchItemStatus, BatchJobStatus, MediaItem } from "./types";

/// Mirrors `can_carry_exif` in `src-tauri/src/metadata.rs`. If that list moves,
/// this is the test that should fail rather than the checkbox quietly lying.
describe("metadataFate", () => {
  it("keeps EXIF for the containers the backend can rewrite", () => {
    expect(metadataFate("jpeg")).toBe("kept");
    expect(metadataFate("png")).toBe("kept");
    expect(metadataFate("webp")).toBe("kept");
  });

  it("admits AVIF cannot hold it", () => {
    expect(metadataFate("avif")).toBe("dropped");
  });

  it("leaves 'keep the source format' as it is: it depends on the file", () => {
    expect(metadataFate("keep")).toBe("depends-on-source");
  });
});

describe("recovered batch jobs", () => {
  const job = {
    running: false,
    finalized: false,
  } as BatchJobStatus;

  it("finalizes a job that ended while the window was unavailable", () => {
    expect(shouldFinalizeRecoveredJob(job)).toBe(true);
  });

  it("does not finalize running or already finalized jobs", () => {
    expect(shouldFinalizeRecoveredJob({ ...job, running: true })).toBe(false);
    expect(shouldFinalizeRecoveredJob({ ...job, finalized: true })).toBe(false);
  });
});

describe("failure reports", () => {
  it("includes both the source path and the actual error", () => {
    const item = {
      file_name: "clip.mov",
      source_path: "D:\\camera\\clip.mov",
      error: "encoder exited with status 1",
    } as BatchItemStatus;

    expect(formatFailureReport([item])).toBe(
      "clip.mov\nD:\\camera\\clip.mov\nencoder exited with status 1",
    );
  });
});

describe("saved batch settings", () => {
  it("migrates settings saved before hardware acceleration existed", () => {
    const oldSettings = {
      video: { codec: "h265", crf: 28 },
      image: { format: "jpeg", quality: 85 },
      output: { mode: "subfolder", name: "_optimized" },
    } as never;

    expect(sanitizeStoredSettings(oldSettings).video.hardware_acceleration).toBe("auto");
  });
});

describe("folder ownership", () => {
  const item = (path: string) => ({ id: path, paths: [path] }) as MediaItem;

  it("matches regardless of separator and case", () => {
    expect(itemIsInsideFolder(item("D:\\camera\\trip\\a.jpg"), "d:/CAMERA")).toBe(true);
    expect(itemIsInsideFolder(item("D:/camera/a.jpg"), "D:\\camera\\")).toBe(true);
  });

  it("does not match a sibling folder with a shared prefix", () => {
    expect(itemIsInsideFolder(item("D:\\camera2\\a.jpg"), "D:\\camera")).toBe(false);
  });

  it("matches a Live Photo through either of its paths", () => {
    const pair = {
      id: "pair",
      paths: ["D:\\other\\a.heic", "D:\\camera\\a.mov"],
    } as MediaItem;
    expect(itemIsInsideFolder(pair, "D:\\camera")).toBe(true);
  });
});

describe("applyRescan", () => {
  const item = (path: string) => ({ id: path, paths: [path] }) as MediaItem;
  const root = "D:\\camera";

  it("adds subfolder files and selects them", () => {
    const current = [item("D:\\camera\\a.jpg")];
    const rescanned = [item("D:\\camera\\a.jpg"), item("D:\\camera\\trip\\b.jpg")];

    const result = applyRescan(current, [root], rescanned, new Set(["D:\\camera\\a.jpg"]));

    expect(result.items).toHaveLength(2);
    expect(result.selected.size).toBe(2);
  });

  it("drops subfolder files again when recursion is turned off", () => {
    const current = [item("D:\\camera\\a.jpg"), item("D:\\camera\\trip\\b.jpg")];
    const rescanned = [item("D:\\camera\\a.jpg")];

    const result = applyRescan(current, [root], rescanned, new Set(current.map((i) => i.id)));

    expect(result.items.map((i) => i.id)).toEqual(["D:\\camera\\a.jpg"]);
  });

  it("keeps files added outside the scanned folders, deselected ones included", () => {
    const outside = item("D:\\downloads\\c.jpg");
    const current = [outside, item("D:\\camera\\a.jpg")];
    const rescanned = [item("D:\\camera\\a.jpg")];

    const result = applyRescan(current, [root], rescanned, new Set(["D:\\camera\\a.jpg"]));

    expect(result.items.map((i) => i.id)).toContain(outside.id);
    expect(result.selected.has(outside.id)).toBe(false);
  });
});

describe("shouldReseedQueue", () => {
  const base = {
    hasQueue: true,
    jobRunning: false,
    hasInitialItems: false,
    itemCount: 0,
    seededFolder: null as string | null,
    queueFolder: "D:/camera" as string | null,
  };

  it("seeds an empty panel from the folder the editor has open", () => {
    expect(shouldReseedQueue(base)).toBe(true);
  });

  it("reseeds when the editor moved to another folder", () => {
    expect(
      shouldReseedQueue({ ...base, itemCount: 5, seededFolder: "D:/old" }),
    ).toBe(true);
  });

  it("keeps what the user assembled while the editor stayed put", () => {
    expect(
      shouldReseedQueue({ ...base, itemCount: 5, seededFolder: "D:/camera" }),
    ).toBe(false);
  });

  it("leaves a single handed-in file alone", () => {
    expect(shouldReseedQueue({ ...base, hasInitialItems: true })).toBe(false);
  });

  it("never touches the list under a running job", () => {
    expect(shouldReseedQueue({ ...base, jobRunning: true })).toBe(false);
  });

  it("does nothing when the editor has no folder open", () => {
    expect(shouldReseedQueue({ ...base, hasQueue: false })).toBe(false);
  });
});
