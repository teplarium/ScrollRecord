/**
 * Locale integrity tests.
 * Why: translators are not programmers. These checks catch a missing key, a
 * broken `{{placeholder}}` or an untranslated error code before release.
 */
import { describe, expect, it } from "vitest";

import errorCodes from "../shared/ipc/generated/error-codes.json";
import { DEFAULT_LANGUAGE, LOCALES } from "./locales";

/** Flattens `{ a: { b: "x" } }` into `{ "a.b": "x" }`. */
function flatten(obj: object, prefix = ""): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(obj)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value && typeof value === "object") Object.assign(out, flatten(value, path));
    else out[path] = String(value);
  }
  return out;
}

/** Collects `{{name}}` placeholders of a message. */
function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map((m) => m[1] ?? "").sort();
}

const english = flatten(LOCALES[DEFAULT_LANGUAGE] ?? {});

describe("locales", () => {
  it("bundles English", () => {
    expect(Object.keys(english).length).toBeGreaterThan(0);
  });

  it("has English text for every Rust error code", () => {
    const missing = errorCodes.filter((code) => !(`errors.${code}` in english));
    expect(missing).toEqual([]);
  });

  for (const [code, messages] of Object.entries(LOCALES)) {
    if (code === DEFAULT_LANGUAGE) continue;
    const translated = flatten(messages);

    it(`${code}: has every English key`, () => {
      const missing = Object.keys(english).filter((key) => !(key in translated));
      expect(missing).toEqual([]);
    });

    it(`${code}: keeps the same {{placeholders}}`, () => {
      for (const [key, text] of Object.entries(translated)) {
        const source = english[key];
        if (source !== undefined) expect(placeholders(text), key).toEqual(placeholders(source));
      }
    });
  }
});
