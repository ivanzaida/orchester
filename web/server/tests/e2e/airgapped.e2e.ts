import { expect, test, type Page } from "@playwright/test";

import { joinWithCode } from "./accounts";
import { adminConnections, PORTS, SERVER, startStack, type Stack } from "./stack";
import { createProject, expectNoSecretIn, inviteWithCode, logs, setUp } from "./steps";

/**
 * An air-gapped team server: no Orchester Cloud at all, none named to the admin. Setup stops after the first admin,
 * people join with codes, and nothing reaches outside: the browser asks only the team server, and the admin's own
 * server side connects only to the server's private API listener.
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

test("a project, and a member joining with a code", async () => {
  await createProject(page);
  const code = await inviteWithCode(page);
  const token = await joinWithCode(code, "oleh", "oleh-pc");
  expect(token).toMatch(/^orc_/);
  expectNoSecretIn(logs(stack), [stack.setupCode, code]);
});

test("nothing reached outside the team server", () => {
  expect(seen.length).toBeGreaterThan(0);
  expect(seen.filter((url) => !url.startsWith(`${SERVER}/`))).toEqual([]);
  // The server's private API listener, and the admin app itself (Next.js asks its own server for some pages).
  const allowed = new Set([`127.0.0.1:${PORTS.serverApi}`, `127.0.0.1:${PORTS.admin}`]);
  expect(adminConnections(stack).filter((address) => !allowed.has(address))).toEqual([]);
});
