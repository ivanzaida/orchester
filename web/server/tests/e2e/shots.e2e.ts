import { test, type Browser, type Page } from "@playwright/test";

import { cloudAccount, joinWithCode } from "./accounts";
import { SERVER, startStack, type Stack } from "./stack";
import { createProject, inviteWithCode, setUp, signInAtCloud } from "./steps";

/**
 * Screenshots of every admin page on the real team server, at the frames' 1440 and a phone's 390, for the design
 * comparison. Only when `SHOTS` names a folder: `SHOTS=<folder> npm run test:e2e -- shots`.
 */
const folder = process.env.SHOTS;

test.skip(folder === undefined, "screenshots only when SHOTS is set");
test.describe.configure({ mode: "serial" });

let stack: Stack;
let page: Page;

async function shoot(name: string, path: string): Promise<void> {
  for (const width of [1440, 390]) {
    await page.setViewportSize({ width, height: width === 1440 ? 900 : 844 });
    await page.goto(`${SERVER}${path}`);
    await page.screenshot({ path: `${String(folder)}/real-${name}-${String(width)}.png`, fullPage: true });
  }
}

async function open(browser: Browser): Promise<Page> {
  const context = await browser.newContext({ ignoreHTTPSErrors: true, colorScheme: "dark" });
  return context.newPage();
}

test.beforeAll(async ({ browser }) => {
  test.setTimeout(300_000);
  stack = await startStack({ cloud: true });
  await cloudAccount(stack, "ivan@example.test", "Ivan Zaida");
  page = await open(browser);
});

test.afterAll(async () => {
  await stack.stop();
});

test("setup and sign-in pages", async ({ browser }) => {
  await shoot("setup-code", "/admin/setup");
  await setUp(page, stack.setupCode);
  await shoot("setup-cloud", "/admin/setup/cloud");
  await page.getByRole("button", { name: "Link to Orchester Cloud" }).click();
  await signInAtCloud(page, "ivan@example.test");
  await page.getByRole("button", { name: "Link Kontur team" }).click();
  await page.waitForURL(/\/admin\/setup\/done$/);
  await shoot("setup-done", "/admin/setup/done");
  const visitor = await open(browser);
  for (const [name, path] of [["signin", "/admin/signin"], ["signin-code", "/admin/signin/code"], ["address", "/kontur"]]) {
    for (const width of [1440, 390]) {
      await visitor.setViewportSize({ width, height: width === 1440 ? 900 : 844 });
      await visitor.goto(`${SERVER}${String(path)}`);
      await visitor.screenshot({ path: `${String(folder)}/real-${String(name)}-${String(width)}.png`, fullPage: true });
    }
  }
});

test("the admin's pages with a project, people and a computer", async () => {
  await shoot("new-project", "/admin/projects/new");
  await createProject(page);
  const code = await inviteWithCode(page);
  await joinWithCode(code, "oleh", "oleh-pc");
  await inviteWithCode(page);
  for (const [name, path] of [["projects", "/admin"], ["members", "/admin/projects/kontur"],
    ["invites", "/admin/projects/kontur/invites"], ["project-settings", "/admin/projects/kontur/settings"],
    ["people", "/admin/people"], ["devices", "/admin/devices"], ["settings", "/admin/settings"]] as const) {
    await shoot(name, path);
  }
});
