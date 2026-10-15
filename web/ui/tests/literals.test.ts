import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { CSS_LITERALS, literalViolations } from "../testing/literals";
import { sources } from "../testing/sources";

const src = sources(fileURLToPath(new URL("..", import.meta.url)), ["components", "hooks", "i18n", "theme"]);
const THEME = ["theme/variables.css", "theme/scale.css", "theme/fonts.css", "theme/base.css"];

describe("design values live only in the theme files", () => {
  it("the UI kit's style sheets and code use tokens", () => {
    const { sheets, found } = literalViolations(src, THEME);
    expect(sheets).toBeGreaterThan(5);
    expect(found).toEqual([]);
  });

  it("the check notices a literal", () => {
    const rules = CSS_LITERALS.map(([pattern]) => pattern);
    for (const sample of ["color: #fff;", "padding: 12px;", "font-family: Arial;", "opacity: 0.5;", "gap: 1.5rem;"]) {
      expect(rules.some((pattern) => pattern.test(sample)), sample).toBe(true);
    }
    for (const sample of ["padding: var(--space-md);", "width: 100%;", "border: 0;", "flex: 1;", "grid-auto-columns: 1fr;"]) {
      expect(rules.some((pattern) => pattern.test(sample)), sample).toBe(false);
    }
  });
});
