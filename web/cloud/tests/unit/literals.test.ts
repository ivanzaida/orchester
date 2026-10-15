import { describe, expect, it } from "vitest";

import { literalViolations } from "@orchester/ui/testing/literals";

import { src, THEME_FILES } from "./sources";

describe("design values live only in the theme files", () => {
  it("the Cloud's style sheets and code use tokens", () => {
    const { sheets, found } = literalViolations(src, THEME_FILES);
    expect(sheets).toBeGreaterThan(0);
    expect(found).toEqual([]);
  });
});
