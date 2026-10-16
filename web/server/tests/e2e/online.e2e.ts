import { expect, test, type Browser, type Page } from "@playwright/test";

import { cloudAccount, cloudDevice, cloudInvites, cloudTwoFactor, joinWithCode, tokenWorks } from "./accounts";
import { SERVER, startStack, type Stack } from "./stack";
import { accountCode, createProject, expectNoSecretIn, inviteWithCode, logs, setUp, signInAtCloud, signInWithCode } from "./steps";

/**
 * The server admin end to end, against a loopback team server and a loopback Orchester Cloud on a throwaway
 * PostgreSQL: setup with the setup code and linking, sign-in through the cloud (with two-factor authentication) and with
 * a code, a project, invites by email (only from the two-factor sign-in, reaching the cloud) and by code, a role, a
 * device revoked, join approval on; and no code or token in a log.
 */

test.describe.configure({ mode: "serial" });

let stack: Stack;
let admin: Page;
/** A browser signed in to the admin through the cloud with two-factor authentication, which sends invites by email. */
let cloudAdmin: Page | undefined;
/** Ivan's recovery codes: each finishes one of his cloud sign-ins in the browser. */
const recovery: string[] = [];
const codes: string[] = [];
let olehToken = "";

async function freshPage(browser: Browser): Promise<Page> {
  const context = await browser.newContext({ ignoreHTTPSErrors: true, colorScheme: "dark" });
  return context.newPage();
}

test.beforeAll(async ({ browser }) => {
  test.setTimeout(300_000);
  stack = await startStack({ cloud: true });
  codes.push(stack.setupCode);
  await cloudAccount(stack, "ivan@example.test", "Ivan Zaida");
  await cloudAccount(stack, "dana@example.test", "Dana Ruiz");
  recovery.push(...(await cloudTwoFactor("ivan@example.test")));
  codes.push(...recovery);
  admin = await freshPage(browser);
});

test.afterAll(async () => {
  await stack.stop();
});

test("setup with the setup code, then linking through the cloud", async () => {
  await setUp(admin, stack.setupCode);
  await admin.getByRole("button", { name: "Link to Orchester Cloud" }).click();
  await signInAtCloud(admin, "ivan@example.test", recovery[0]);
  await expect(admin.getByRole("heading", { name: "Link Kontur team to your account?" })).toBeVisible();
  await admin.getByRole("button", { name: "Link Kontur team" }).click();
  await expect(admin).toHaveURL(/\/admin\/setup\/done$/);
  await expect(admin.getByText("Kontur team is ready")).toBeVisible();
  await expect(admin.getByText("Linked by Ivan Zaida")).toBeVisible();
});

test("sign-in through the cloud", async ({ browser }) => {
  const page = await freshPage(browser);
  await page.goto(`${SERVER}/admin`);
  await expect(page).toHaveURL(/\/admin\/signin$/);
  await page.getByRole("button", { name: "Sign in with Orchester Cloud" }).click();
  await signInAtCloud(page, "ivan@example.test", recovery[1]);
  await page.getByRole("button", { name: "Sign in to the admin" }).click();
  await expect(page).toHaveURL(`${SERVER}/admin`);
  await expect(page.getByRole("heading", { name: "Projects", exact: true })).toBeVisible();
  // Kept for the invites by email, which only a cloud sign-in with two-factor authentication sends.
  cloudAdmin = page;
});

test("sign-in with an account code from the host", async ({ browser }) => {
  const code = await accountCode(stack, "ivan");
  codes.push(code);
  const page = await freshPage(browser);
  await signInWithCode(page, code);
  await page.context().close();
});

test("creating a project and inviting by email: the invite reaches the cloud", async () => {
  await createProject(admin);
  await admin.goto(`${SERVER}/admin/projects/kontur/invites`);
  await admin.getByRole("textbox", { name: "Email" }).fill("dana@example.test");
  await admin.getByRole("button", { name: "Send invite" }).click();
  // The setup code's sign-in did not come through the cloud with two-factor authentication.
  await expect(admin.getByText("Two-factor authentication needed")).toBeVisible();
  const sender = cloudAdmin ?? admin;
  await sender.goto(`${SERVER}/admin/projects/kontur/invites`);
  await sender.getByRole("textbox", { name: "Email" }).fill("dana@example.test");
  await sender.getByRole("button", { name: "Send invite" }).click();
  await expect(sender.getByText("Invite sent to dana@example.test")).toBeVisible();
  const invites = await cloudInvites(await cloudDevice("dana@example.test"));
  expect(invites).toHaveLength(1);
  expect(invites[0]?.server).toMatchObject({ name: "Kontur team", address: SERVER });
  expect(invites[0]?.project).toEqual({ name: "Kontur", address: `${SERVER}/kontur` });
});

test("inviting by code, and changing the new member's role", async () => {
  const code = await inviteWithCode(admin);
  codes.push(code);
  olehToken = await joinWithCode(code, "oleh", "oleh-pc");
  await admin.goto(`${SERVER}/admin/projects/kontur`);
  await admin.getByLabel("Role of oleh").selectOption("owner");
  await expect(admin.getByLabel("Role of oleh")).toHaveValue("owner");
  await admin.reload();
  await expect(admin.getByLabel("Role of oleh")).toHaveValue("owner");
});

test("revoking a device", async () => {
  expect(await tokenWorks(olehToken)).toBe(true);
  await admin.goto(`${SERVER}/admin/devices`);
  // Oleh's is the only computer; the browsers signed in to the admin are listed apart (security review 1, minor 5).
  const joined = admin.getByRole("region", { name: "Joined" });
  await expect(joined.getByText("Orchester on oleh-pc")).toBeVisible();
  await expect(admin.getByRole("region", { name: "Signed in to this admin" }).getByText("This browser")).toBeVisible();
  await joined.getByRole("button", { name: "Revoke" }).click();
  await joined.getByRole("button", { name: "Revoke" }).last().click();
  await expect(admin.getByText("Orchester on oleh-pc is signed out")).toBeVisible();
  expect(await tokenWorks(olehToken)).toBe(false);
});

test("turning join approval on", async () => {
  await admin.goto(`${SERVER}/admin/settings`);
  const approval = admin.getByRole("switch", { name: "Join approval" });
  await expect(approval).toHaveAttribute("aria-checked", "false");
  await approval.click();
  await expect(approval).toHaveAttribute("aria-checked", "true");
  await admin.reload();
  await expect(admin.getByRole("switch", { name: "Join approval" })).toHaveAttribute("aria-checked", "true");
});

test("no code or token reaches a log", () => {
  expectNoSecretIn(logs(stack), codes);
});
