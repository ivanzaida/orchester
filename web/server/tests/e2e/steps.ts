import { expect, type Page } from "@playwright/test";

import { PASSWORD } from "./accounts";
import { run } from "./processes";
import { BINARY, SERVER, type Stack } from "./stack";

/** What a person does in the browser in the end-to-end run, step by step, in the admin's own words (`locales/en.json`). */

export async function signInAtCloud(page: Page, email: string): Promise<void> {
  await expect(page).toHaveURL(/localhost:4400\/signin\?next=/);
  // The form's fields are React's once the page has hydrated; typing before that is lost.
  await page.waitForLoadState("networkidle");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
}

export async function setUp(page: Page, setupCode: string): Promise<void> {
  await page.goto(`${SERVER}/admin`);
  await expect(page).toHaveURL(/\/admin\/setup$/);
  await page.getByLabel("Setup code").fill(setupCode);
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page).toHaveURL(/\/admin\/setup\/account$/);
  await page.getByLabel("Server name").fill("Kontur team");
  await page.getByLabel("Username").fill("ivan");
  await page.getByLabel("Display name").fill("Ivan Zaida");
  await page.getByRole("button", { name: "Continue" }).click();
  await expect(page).toHaveURL(/\/admin\/setup\/cloud$/);
}

/** The host command an admin runs at the server for an account code (`orchester-server account-code`). */
export async function accountCode(stack: Stack, username: string): Promise<string> {
  const printed = await run(BINARY.server, ["account-code", username], stack.serverConfig);
  const code = /Account code: ([0-9A-Z]{4}-[0-9A-Z]{4})/.exec(printed)?.[1];
  if (code === undefined) {
    throw new Error("orchester-server account-code printed no code");
  }
  return code;
}

export async function signInWithCode(page: Page, code: string): Promise<void> {
  await page.goto(`${SERVER}/admin/signin/code`);
  await page.getByLabel("Code").fill(code);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
}

export async function createProject(page: Page): Promise<void> {
  await page.goto(`${SERVER}/admin/projects/new`);
  await page.getByLabel("Name").fill("Kontur");
  await page.getByLabel("Slug").fill("kontur");
  await page.getByLabel("Repository hint · optional").fill("https://github.com/acme/kontur.git");
  await page.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByText("Kontur is created")).toBeVisible();
}

/** *Invite with a code* on a project's Invites: the code, shown once. */
export async function inviteWithCode(page: Page): Promise<string> {
  await page.goto(`${SERVER}/admin/projects/kontur/invites`);
  await page.getByRole("button", { name: "Make a code" }).click();
  const code = page.locator("output").first();
  await expect(code).toHaveText(/^[0-9A-Z]{4}-[0-9A-Z]{4}$/);
  return (await code.textContent()) ?? "";
}

/**
 * Every log line the run's services wrote: the services' logs (stderr), and everything the two web apps wrote. The
 * team server's console (its stdout) prints the setup code on purpose and is left out.
 */
export function logs(stack: Stack): string {
  return stack.processes
    .map((started) => (started.name === "orchester-server" ? started.stderr() : `${started.stdout()}\n${started.stderr()}`))
    .join("\n");
}

/** No code, device token or identity token in the logs. */
export function expectNoSecretIn(text: string, codes: readonly string[]): void {
  for (const code of codes) {
    expect(text.includes(code) || text.includes(code.replace("-", "")), `a code in the logs`).toBe(false);
  }
  expect(text).not.toMatch(/orcc?_[0-9a-f]{64}/);
  expect(text).not.toMatch(/eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\./);
}
