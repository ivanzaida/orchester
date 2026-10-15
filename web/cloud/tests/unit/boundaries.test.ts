import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { relativePath, sourceFiles } from "./sources";

/**
 * The browser never holds an API token or calls the API (`docs/architecture/server.md`, "Who calls the API"): code
 * that runs in the browser (`"use client"` modules and the shared UI kit) never imports the server-side modules, which
 * hold the API client, the configuration, the secret and the session.
 */
function imports(file: string): string[] {
  return [...readFileSync(file, "utf8").matchAll(/from\s+"([^"]+)"/g)].map((match) => match[1] ?? "");
}

function runsInBrowser(file: string): boolean {
  const text = readFileSync(file, "utf8");
  return /^["']use client["'];/m.test(text) || relativePath(file).startsWith("ui/");
}

describe("browser code", () => {
  const browserFiles = sourceFiles([".ts", ".tsx"]).filter(runsInBrowser);

  it("exists", () => {
    expect(browserFiles.length).toBeGreaterThan(5);
  });

  it("never imports the server-side modules", () => {
    const leaks = browserFiles.flatMap((file) =>
      imports(file)
        .filter((path) => path.includes("lib/server") || path.startsWith("node:") || path === "next/headers")
        .map((path) => `${relativePath(file)} imports ${path}`),
    );
    expect(leaks).toEqual([]);
  });

  it("reaches the server only through server actions", () => {
    for (const file of sourceFiles([".ts"]).filter((path) => relativePath(path).startsWith("app/") && path.endsWith("actions.ts"))) {
      expect(readFileSync(file, "utf8").startsWith('"use server";'), relativePath(file)).toBe(true);
    }
  });

  it("has no fetch of its own", () => {
    const fetching = browserFiles.filter((file) => /\bfetch\(/.test(readFileSync(file, "utf8")));
    expect(fetching.map(relativePath)).toEqual([]);
  });
});
