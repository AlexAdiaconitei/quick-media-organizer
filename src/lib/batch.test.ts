import { describe, expect, it } from "vitest";

import { metadataFate } from "./batch";

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
