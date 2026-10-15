import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { generate, OUTPUT_FILE, read } from "../scripts/tokens";

describe("design tokens", () => {
  it("theme/variables.css is what the generator makes from the Pencil snapshot", () => {
    expect(readFileSync(OUTPUT_FILE, "utf8").replaceAll("\r\n", "\n")).toBe(generate(read()));
  });

  it("carries every variable, dark first, light under prefers-color-scheme", () => {
    const css = generate(read());
    expect(css).toContain("--color-surface-base: #111113;");
    expect(css).toContain("--color-surface-base: #e4e4e8;");
    expect(css).toContain("--space-section: 32px;");
    expect(css).toContain("--control-lg: 36px;");
    expect(css).toContain('--font-sans: "Geist", system-ui, sans-serif;');
    expect(css.indexOf("color-scheme: dark")).toBeLessThan(css.indexOf("prefers-color-scheme: light"));
  });

  it("refuses a number variable it has no CSS name for", () => {
    const snapshot = { source: "", variables: { opacity_hover: { type: "number" as const, value: 1 } } };
    expect(() => generate(snapshot)).toThrow(/opacity_hover/);
  });
});
