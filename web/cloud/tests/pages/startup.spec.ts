import { spawn } from "node:child_process";
import { createRequire } from "node:module";

import { expect, test } from "@playwright/test";

import { TEST_ENV } from "../../playwright.config";

/** Starts the built site with `env` and waits up to 30 s for it to exit; `null` when it keeps running. */
function exitOf(env: Record<string, string>): Promise<{ code: number | null; log: string }> {
  return new Promise((resolve) => {
    // Node runs Next's own script, with no shell between, so `kill` reaches the server itself.
    const next = createRequire(import.meta.url).resolve("next/dist/bin/next");
    const server = spawn(process.execPath, [next, "start", "-H", "127.0.0.1", "-p", "4199"], {
      env: { ...process.env, ...env },
    });
    let log = "";
    server.stdout.on("data", (chunk: Buffer) => (log += chunk.toString()));
    server.stderr.on("data", (chunk: Buffer) => (log += chunk.toString()));
    const timer = setTimeout(() => {
      server.kill();
      resolve({ code: null, log });
    }, 30_000);
    server.on("exit", (code) => {
      clearTimeout(timer);
      resolve({ code, log });
    });
  });
}

test.describe("startup", () => {
  test("a wrong secret stops the server with the variable's name, rather than serving errors", async () => {
    test.setTimeout(60_000);
    const { code, log } = await exitOf({ ...TEST_ENV, ORCHESTER_CLOUD_WEB_SECRET: "not base64!!" });
    expect(code).not.toBeNull();
    expect(code).not.toBe(0);
    expect(log).toContain("ORCHESTER_CLOUD_WEB_SECRET");
  });
});
