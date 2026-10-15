import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { LOCALES, t, type MessageKey } from "@/lib/i18n";

import { sourceFiles, ROOT } from "./sources";

type Tree = { [key: string]: string | Tree };

function flatten(tree: Tree, prefix = ""): Map<string, string> {
  const result = new Map<string, string>();
  for (const [key, value] of Object.entries(tree)) {
    const path = `${prefix}${key}`;
    if (typeof value === "string") {
      result.set(path, value);
    } else {
      for (const [inner, text] of flatten(value, `${path}.`)) {
        result.set(inner, text);
      }
    }
  }
  return result;
}

function placeholders(text: string): string[] {
  return [...text.matchAll(/\{\{(\w+)\}\}/g)].map((match) => match[1] ?? "").sort();
}

const english = flatten(JSON.parse(readFileSync(join(ROOT, "locales/en.json"), "utf8")) as Tree);

describe("locales", () => {
  const files = readdirSync(join(ROOT, "locales")).filter((name) => name.endsWith(".json"));

  it("ship exactly the locales the app loads", () => {
    expect(files.map((name) => name.replace(/\.json$/, "")).sort()).toEqual(Object.keys(LOCALES).sort());
  });

  it.each(files)("%s has every key of en.json with the same placeholders, and no others", (name) => {
    const locale = flatten(JSON.parse(readFileSync(join(ROOT, "locales", name), "utf8")) as Tree);
    expect([...locale.keys()].sort()).toEqual([...english.keys()].sort());
    for (const [key, text] of english) {
      expect(placeholders(locale.get(key) ?? ""), key).toEqual(placeholders(text));
    }
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
  for (const file of sourceFiles([".ts", ".tsx"])) {
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
