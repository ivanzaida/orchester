import { expect, test, type Page } from "@playwright/test";

import { adminConnections, PORTS, SERVER, SERVER_API, startStack, type Stack } from "./stack";
import { accountCode, createProject, expectNoSecretIn, logs, setUp } from "./steps";

/**
 * An air-gapped team server: no Orchester Cloud at all, none named to the admin. Setup stops after the first admin, and
 * nothing reaches outside: the browser asks only the team server, and the admin's own server side connects only to the
 * server's private API listener. With no cloud root pinned the server cannot check two-factor authentication, which
 * making an invite and entering a code need: it refuses both and says so (`docs/architecture/server.md`, "Codes").
 */

test.describe.configure({ mode: "serial" });

let stack: Stack;
let page: Page;
const seen: string[] = [];

test.beforeAll(async ({ browser }) => {
  test.setTimeout(300_000);
  stack = await startStack({ cloud: false });
  const context = await browser.newContext({ ignoreHTTPSErrors: true, colorScheme: "dark" });
  page = await context.newPage();
  page.on("request", (request) => seen.push(request.url()));
});

test.afterAll(async () => {
  await stack.stop();
});

test("setup without the internet stops after the first admin", async () => {
  await setUp(page, stack.setupCode);
  await expect(page.getByRole("button", { name: "Link to Orchester Cloud" })).toHaveCount(0);
  await page.getByRole("link", { name: "Not now" }).or(page.getByRole("button", { name: "Not now" })).click();
  await expect(page.getByText("Kontur team is ready")).toBeVisible();
  await expect(page.getByText("Not linked to Orchester Cloud")).toBeVisible();
});

test("a project; invites and codes wait for two-factor authentication the server can check", async () => {
  await createProject(page);
  await page.goto(`${SERVER}/admin/projects/kontur/invites`);
  await page.getByRole("button", { name: "Make a code" }).click();
  await expect(page.getByText("Making invites, codes too, needs two-factor authentication", { exact: false })).toBeVisible();
  const code = await accountCode(stack, "ivan");
  const refused = await fetch(`${SERVER_API}/auth/code`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ code, device: "ivan-pc" }),
  });
  expect(refused.status).toBe(403);
  expect(((await refused.json()) as { error?: unknown }).error).toBe("mfa_unverifiable");
  expectNoSecretIn(logs(stack), [stack.setupCode, code]);
});

test("nothing reached outside the team server", () => {
  expect(seen.length).toBeGreaterThan(0);
  expect(seen.filter((url) => !url.startsWith(`${SERVER}/`))).toEqual([]);
  // The server's private API listener, and the admin app itself (Next.js asks its own server for some pages).
  const allowed = new Set([`127.0.0.1:${PORTS.serverApi}`, `127.0.0.1:${PORTS.admin}`]);
  expect(adminConnections(stack).filter((address) => !allowed.has(address))).toEqual([]);
});
