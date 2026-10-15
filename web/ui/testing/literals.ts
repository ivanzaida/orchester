import { readFileSync } from "node:fs";

import type { Sources } from "./sources";

/**
 * Rule 13 for the web: every colour, font, size, spacing, radius and border comes from the theme files. Anywhere
 * else a value with a unit, a colour or a font name is a violation. `0`, percentages, `vh`, `fr`, `turn` and layout
 * keywords are allowed; they are not design values.
 */
export const CSS_LITERALS: readonly [RegExp, string][] = [
  [/#[0-9a-fA-F]{3,8}\b/, "a colour"],
  [/\b(?:rgba?|hsla?|oklch|color-mix)\(/, "a colour"],
  [/(?<![\w-])\d*\.?\d+(?:px|rem|em|pt|ch)\b/, "a size"],
  [/font-family:(?!\s*var\()/, "a font"],
  [/font-weight:\s*\d/, "a weight"],
  [/(?:font-size|line-height|letter-spacing|opacity):\s*\d/, "a type or state value"],
  [/(?<![\w-])(?:white|black|red|green|blue|gray|grey)(?![\w-])/, "a named colour"],
];

export const CODE_LITERALS: readonly [RegExp, string][] = [
  [/["'`]#[0-9a-fA-F]{3,8}["'`]/, "a colour"],
  [/["'`][^"'`]*\d(?:px|rem|em)\b/, "a size"],
];

function violations(file: string, label: string, rules: readonly [RegExp, string][]): string[] {
  return readFileSync(file, "utf8")
    .split("\n")
    .flatMap((line, index) => {
      const code = line.replace(/\/\*.*?\*\//g, "").replace(/\/\/.*$/, "");
      return rules.filter(([pattern]) => pattern.test(code)).map(([, what]) => `${label}:${index + 1} ${what}: ${line.trim()}`);
    });
}

/** Every literal outside the theme files (`themeFiles`, relative paths) in a package's CSS and code. */
export function literalViolations(src: Sources, themeFiles: string[]): { sheets: number; found: string[] } {
  const sheets = src.files([".css"]).filter((file) => !themeFiles.includes(src.relative(file)));
  const found = [
    ...sheets.flatMap((file) => violations(file, src.relative(file), CSS_LITERALS)),
    ...src.files([".ts", ".tsx"]).flatMap((file) => violations(file, src.relative(file), CODE_LITERALS)),
  ];
  return { sheets: sheets.length, found };
}
