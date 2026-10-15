import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { createTranslator, parityProblems } from "../i18n";
import { sources } from "../testing/sources";

const root = fileURLToPath(new URL("..", import.meta.url));
const messages = { en: { a: { title: "Hello {{name}}" } }, de: { a: { title: "Hallo {{name}}" } } };

describe("translator", () => {
  const { t } = createTranslator<typeof messages, "a.title">(messages, "en");

  it("fills placeholders in the asked locale", () => {
    expect(t("a.title", { name: "Mira" })).toBe("Hello Mira");
    expect(t("a.title", { name: "Mira" }, "de")).toBe("Hallo Mira");
  });

  it("refuses a missing value or key instead of showing it raw", () => {
    expect(() => t("a.title")).toThrow(/name/);
    expect(() => t("a.other" as "a.title")).toThrow(/missing/);
  });
});

describe("locale parity", () => {
  it("names missing, extra and changed keys", () => {
    expect(parityProblems({ a: "x {{n}}", b: "y" }, { a: "x", c: "z" })).toEqual([
      "placeholders differ in a",
      "missing b",
      "extra c",
    ]);
    expect(parityProblems(messages.en, messages.de)).toEqual([]);
  });
});

describe("the UI kit has no words of its own", () => {
  it("ships no locale files and reads none", () => {
    expect(existsSync(`${root}/locales`)).toBe(false);
    const code = sources(root, ["components", "hooks"]).files([".ts", ".tsx"]);
    expect(code.length).toBeGreaterThan(5);
    for (const file of code) {
      expect(readFileSync(file, "utf8"), file).not.toMatch(/locales|from "@\//);
    }
  });
});
