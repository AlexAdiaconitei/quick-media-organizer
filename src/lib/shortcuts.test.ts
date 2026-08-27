import { afterEach, describe, expect, it, vi } from "vitest";

import { isSkipShortcut, modKey, modLabel, skipModLabel } from "./shortcuts";

/// The module reads `navigator.platform`, so the platform is faked per test
/// rather than assuming whatever machine runs the suite.
function onPlatform(platform: string) {
  vi.stubGlobal("navigator", { platform });
}

/// Only the fields the shortcut helpers look at.
function keyEvent(init: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    key: "",
    metaKey: false,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    ...init,
  } as KeyboardEvent;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("modLabel", () => {
  it("names the Windows modifier", () => {
    onPlatform("Win32");
    expect(modLabel("z")).toBe("Ctrl+Z");
  });

  it("names the macOS modifier", () => {
    onPlatform("MacIntel");
    expect(modLabel("z")).toBe("⌘Z");
  });
});

describe("modKey", () => {
  it("is null without a modifier, so plain typing is never a shortcut", () => {
    onPlatform("Win32");
    expect(modKey(keyEvent({ key: "d" }))).toBeNull();
  });

  it("lower-cases the key so Ctrl+Shift+D still resolves", () => {
    onPlatform("Win32");
    expect(modKey(keyEvent({ key: "D", ctrlKey: true }))).toBe("d");
  });
});

describe("isSkipShortcut", () => {
  it("is Ctrl+Space on Windows", () => {
    onPlatform("Win32");
    expect(isSkipShortcut(keyEvent({ key: " ", ctrlKey: true }))).toBe(true);
    expect(skipModLabel()).toBe("Ctrl+Space");
  });

  it("is Cmd+Shift+Space on macOS", () => {
    onPlatform("MacIntel");
    expect(isSkipShortcut(keyEvent({ key: " ", metaKey: true, shiftKey: true }))).toBe(true);
    expect(skipModLabel()).toBe("⌘⇧Space");
  });

  it("ignores a bare space, which belongs to the rename field", () => {
    onPlatform("Win32");
    expect(isSkipShortcut(keyEvent({ key: " " }))).toBe(false);
  });

  it("ignores the other platform's combination", () => {
    onPlatform("Win32");
    expect(isSkipShortcut(keyEvent({ key: " ", metaKey: true, shiftKey: true }))).toBe(false);
  });
});
