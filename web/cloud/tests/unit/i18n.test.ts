import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { LOCALES, t, type MessageKey } from "@/lib/i18n";

import { flatten, parityProblems } from "@orchester/ui/i18n";

import { ROOT, src } from "./sources";

const english = flatten(JSON.parse(readFileSync(join(ROOT, "locales/en.json"), "utf8")) as object);

describe("locales", () => {
  const files = readdirSync(join(ROOT, "locales")).filter((name) => name.endsWith(".json"));

  it("ship exactly the locales the app loads", () => {
    expect(files.map((name) => name.replace(/\.json$/, "")).sort()).toEqual(Object.keys(LOCALES).sort());
  });

  it.each(files)("%s has every key of en.json with the same placeholders, and no others", (name) => {
    const reference = JSON.parse(readFileSync(join(ROOT, "locales/en.json"), "utf8")) as object;
    const locale = JSON.parse(readFileSync(join(ROOT, "locales", name), "utf8")) as object;
    expect(parityProblems(reference, locale)).toEqual([]);
  });
});

describe("every key the code names", () => {
  // A dotted lowercase string literal in the code (comments left out) is a message key: t("signin.title"),
  // message("error.refused"), { key: "signin.wrong" }. Imports start with "./", "@/" or a package name without dots.
  const KEY = /"([a-z][a-zA-Z]*(?:\.[a-z][a-zA-Z]*)+)"/g;
  const code = (file: string) =>
    readFileSync(file, "utf8")
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "");
  const used = new Set<string>();
  for (const file of src.files([".ts", ".tsx"])) {
    for (const match of code(file).matchAll(KEY)) {
      used.add(match[1] ?? "");
    }
  }

  it("finds keys at all", () => {
    expect(used.size).toBeGreaterThan(50);
  });

  it.each([...used].sort())("%s exists in en.json", (key) => {
    expect(english.has(key)).toBe(true);
  });
});

describe("t", () => {
  it("fills placeholders", () => {
    expect(t("error.passwordShort", { count: 8 })).toBe("Use at least 12 characters; this one has 8.");
  });

  it("refuses a missing value instead of showing the placeholder", () => {
    expect(() => t("error.passwordShort")).toThrow(/count/);
  });

  it("refuses an unknown key at run time too", () => {
    expect(() => t("signin.nothing" as MessageKey)).toThrow(/missing/);
  });
});
