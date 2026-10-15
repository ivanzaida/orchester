import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { relativePath, sourceFiles } from "./sources";

/**
 * Every colour, font, size, spacing, radius and border comes from `theme/tokens.css` (rule 13 for the web front):
 * anywhere else, a value with a unit, a colour or a font name fails here. `0`, percentages, `vh`, `turn` and plain
 * layout keywords are allowed; they are not design values.
 */
const TOKENS = "theme/tokens.css";

const CSS_LITERALS: [RegExp, string][] = [
  [/#[0-9a-fA-F]{3,8}\b/, "a colour"],
  [/\b(?:rgba?|hsla?|oklch|color-mix)\(/, "a colour"],
  [/(?<![\w-])\d*\.?\d+(?:px|rem|em|pt|ch)\b/, "a size"],
  [/font-family:(?!\s*var\()/, "a font"],
  [/font-weight:\s*\d/, "a weight"],
  [/(?:font-size|line-height|letter-spacing|opacity):\s*\d/, "a type or state value"],
  [/(?<![\w-])(?:white|black|red|green|blue|gray|grey)(?![\w-])/, "a named colour"],
];

const CODE_LITERALS: [RegExp, string][] = [
  [/["'`]#[0-9a-fA-F]{3,8}["'`]/, "a colour"],
  [/["'`][^"'`]*\d(?:px|rem|em)\b/, "a size"],
];

function violations(file: string, rules: [RegExp, string][]): string[] {
  return readFileSync(file, "utf8")
    .split("\n")
    .flatMap((line, index) => {
      const code = line.replace(/\/\*.*?\*\//g, "").replace(/\/\/.*$/, "");
      return rules.filter(([pattern]) => pattern.test(code)).map(([, what]) => `${relativePath(file)}:${index + 1} ${what}: ${line.trim()}`);
    });
}

describe("design values live only in theme/tokens.css", () => {
  it("style sheets use tokens", () => {
    const sheets = sourceFiles([".css"]).filter((file) => relativePath(file) !== TOKENS);
    expect(sheets.length).toBeGreaterThan(5);
    expect(sheets.flatMap((file) => violations(file, CSS_LITERALS))).toEqual([]);
  });

  it("components and code carry no colours or sizes", () => {
    expect(sourceFiles([".ts", ".tsx"]).flatMap((file) => violations(file, CODE_LITERALS))).toEqual([]);
  });

  it("the check notices a literal", () => {
    const rules = CSS_LITERALS.map(([pattern]) => pattern);
    for (const sample of ["color: #fff;", "padding: 12px;", "font-family: Arial;", "opacity: 0.5;", "gap: 1.5rem;"]) {
      expect(rules.some((pattern) => pattern.test(sample)), sample).toBe(true);
    }
    for (const sample of ["padding: var(--space-md);", "width: 100%;", "border: 0;", "flex: 1;"]) {
      expect(rules.some((pattern) => pattern.test(sample)), sample).toBe(false);
    }
  });
});
