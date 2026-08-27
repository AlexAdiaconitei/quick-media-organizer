import { describe, expect, it } from "vitest";

import { format, isLocale, messages, t } from "./i18n";

/// Walks a message table and yields every leaf as a dotted key.
function keysOf(table: unknown, prefix = ""): string[] {
  if (typeof table === "string") return [prefix];
  if (table === null || typeof table !== "object") return [];
  return Object.entries(table as Record<string, unknown>).flatMap(([name, value]) =>
    keysOf(value, prefix ? `${prefix}.${name}` : name),
  );
}

describe("message tables", () => {
  it("cover the same keys in every language", () => {
    const english = keysOf(messages.en).sort();
    const spanish = keysOf(messages.es).sort();

    expect(spanish.filter((key) => !english.includes(key))).toEqual([]);
    expect(english.filter((key) => !spanish.includes(key))).toEqual([]);
  });

  it("use the same placeholders in every language", () => {
    const placeholders = (text: string) =>
      [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();

    for (const key of keysOf(messages.en)) {
      expect(placeholders(t("es", key)), `placeholders differ for ${key}`).toEqual(
        placeholders(t("en", key)),
      );
    }
  });

  /// The backend names these; a rename on either side must not go unnoticed.
  it("carry every message the backend can send", () => {
    const fromBackend = [
      "writeName",
      "action.renamed",
      "action.renamedAdjusted",
      "action.savedToFolder",
      "action.trashed",
      "action.undone",
      "action.trimmed",
      "action.notAVideo",
      "action.trimTooShort",
      "action.trimNothing",
      "action.undoUnavailable",
      "action.undoHistoryTrimmed",
    ];

    for (const key of fromBackend) {
      expect(t("en", key), `missing English text for ${key}`).not.toBe(key);
      expect(t("es", key), `missing Spanish text for ${key}`).not.toBe(key);
    }
  });
});

describe("format", () => {
  it("replaces every occurrence of a placeholder, not just the first", () => {
    expect(format("en", "missing.key.{key}.{key}", { key: "Z" })).toBe("missing.key.Z.Z");
  });

  it("leaves placeholders it was given no value for", () => {
    expect(format("en", "undoHint", {})).toContain("{key}");
  });
});

describe("isLocale", () => {
  it("accepts the languages this build ships", () => {
    expect(isLocale("en")).toBe(true);
    expect(isLocale("es")).toBe(true);
  });

  it("rejects anything else, so a stray settings value cannot blank the UI", () => {
    expect(isLocale("fr")).toBe(false);
    expect(isLocale("")).toBe(false);
    expect(isLocale(undefined)).toBe(false);
    expect(isLocale(null)).toBe(false);
  });
});
