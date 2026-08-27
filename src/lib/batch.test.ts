import { describe, expect, it } from "vitest";

import {
  formatFailureReport,
  metadataFate,
  shouldFinalizeRecoveredJob,
} from "./batch";
import type { BatchItemStatus, BatchJobStatus } from "./types";

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
