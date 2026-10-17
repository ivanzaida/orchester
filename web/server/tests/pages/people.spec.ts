import { expect, test } from "@playwright/test";

import { apiRequests, reset, signIn } from "./support";

const OLEH = "0199f0a0-0000-7000-8000-000000000003";
const DANA = "0199f0a0-0000-7000-8000-000000000005";
const MARIA = "0199f0a0-0000-7000-8000-000000000004";

test.beforeEach(async () => {
  await reset();
});

test.describe("people", () => {
  test("lists the accounts with how they sign in and their role", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await expect(page.getByText("5 people")).toBeVisible();
    await expect(page.getByText("Disabled", { exact: true })).toBeVisible();
    await expect(page.getByText("Codes", { exact: true })).toBeVisible();
  });

  test("making an admin asks in place", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await page.getByRole("button", { name: "Make admin" }).first().click();
    const question = page.getByRole("group", { name: "Make Oleh Petrenko an admin of Kontur team?" });
    await question.getByRole("button", { name: "Make admin" }).click();
    await expect.poll(async () => (await apiRequests()).find((r) => r.method === "PATCH")).toMatchObject({ path: `/api/v1/admin/users/${OLEH}`, body: { admin: true } });
    await expect(question).toHaveCount(0);
  });

  test("disabling asks in place; enabling does not", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await page.getByRole("button", { name: "Disable" }).nth(1).click();
    await expect(page.getByRole("group", { name: "Disable Oleh Petrenko's account?" })).toBeVisible();
    await page.getByRole("group", { name: "Disable Oleh Petrenko's account?" }).getByRole("button", { name: "Keep" }).click();
    await page.getByRole("button", { name: "Enable" }).click();
    await expect.poll(async () => (await apiRequests()).find((r) => r.method === "PATCH")).toMatchObject({ path: `/api/v1/admin/users/${DANA}`, body: { disabled: false } });
  });

  test("unlinking an account's Orchester Cloud asks in place", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    const maria = page.getByRole("row").filter({ hasText: "Maria Lind" });
    await maria.getByRole("button", { name: "Unlink Orchester Cloud" }).click();
    const question = page.getByRole("group", { name: "Unlink Maria Lind's Orchester Cloud account?" });
    await question.getByRole("button", { name: "Unlink Orchester Cloud" }).click();
    await expect.poll(async () => (await apiRequests()).find((r) => r.method === "DELETE")).toMatchObject({ path: `/api/v1/admin/users/${MARIA}/cloud` });
    await expect(maria.getByRole("button", { name: "Unlink Orchester Cloud" })).toHaveCount(0);
    await expect(maria.getByText("Codes", { exact: true })).toBeVisible();
  });

  test("an account code is shown once under the account", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await page.getByRole("button", { name: "Account code" }).first().click();
    await page.getByRole("button", { name: "Account code" }).nth(1).click();
    await expect(page.getByText("Account code for oleh")).toBeVisible();
    await expect(page.getByText("K7QM-3XRD")).toBeVisible();
    const made = (await apiRequests()).find((r) => r.method === "POST" && r.path === "/api/v1/invites");
    expect(made?.body).toMatchObject({ kind: "account", delivery: "code", user: OLEH });
  });

  test("an invite to the server can be for an admin", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await page.getByLabel("Email", { exact: true }).fill("lee@example.com");
    await page.getByLabel("As an admin").check();
    await page.getByRole("button", { name: "Send invite" }).click();
    await expect(page.getByText("Invite sent to lee@example.com, as an admin")).toBeVisible();
    const made = (await apiRequests()).find((r) => r.method === "POST" && r.path === "/api/v1/invites");
    expect(made?.body).toMatchObject({ kind: "server", delivery: "cloud", email: "lee@example.com", admin: true });
  });

  test("an invite code to the server is shown once", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/people");
    await page.getByRole("button", { name: "Invite with a code" }).click();
    await expect(page.getByText("Invite code for Kontur team", { exact: true })).toBeVisible();
    await expect(page.getByText("K7QM-3XRD")).toBeVisible();
  });
});

test.describe("devices", () => {
  test("approves a waiting computer at once", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/devices");
    await page.getByRole("button", { name: "Approve" }).click();
    await expect(page.getByText("Approved by you")).toBeVisible();
    expect((await apiRequests()).find((r) => r.path.startsWith("/api/v1/auth/approvals/"))?.body).toEqual({ approve: true });
  });

  test("refusing asks in place", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/devices");
    await page.getByRole("button", { name: "Refuse" }).click();
    const question = page.getByRole("group", { name: "Refuse Orchester on oleh-pc?" });
    await question.getByRole("button", { name: "Refuse" }).click();
    await expect(page.getByText("Refused", { exact: true })).toBeVisible();
    expect((await apiRequests()).find((r) => r.path.startsWith("/api/v1/auth/approvals/"))?.body).toEqual({ approve: false });
  });

  test("revoking asks in place and signs the computer out", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/devices");
    await page.getByRole("region", { name: "Joined" }).getByRole("button", { name: "Revoke" }).last().click();
    await page.getByRole("group", { name: "Revoke Orchester on build-02?" }).getByRole("button", { name: "Revoke" }).click();
    await expect(page.getByText("Orchester on build-02 is signed out")).toBeVisible();
  });

  test("browsers signed in to the admin are listed, and one is signed out after a question", async ({ page }) => {
    // Security review 1, minor 5.
    await signIn(page);
    await page.goto("/admin/devices");
    const browsers = page.getByRole("region", { name: "Signed in to this admin" });
    await expect(browsers.getByText("3 browsers")).toBeVisible();
    await expect(browsers.getByText("This browser")).toBeVisible();
    await expect(browsers.getByRole("button", { name: "Revoke" })).toHaveCount(2);
    await browsers.getByRole("button", { name: "Revoke" }).last().click();
    await browsers.getByRole("group", { name: "Sign Alice Moreau out of the admin in that browser?" }).getByRole("button", { name: "Revoke" }).click();
    await expect(page.getByText("Alice Moreau's browser is signed out of the admin")).toBeVisible();
    expect((await apiRequests()).some((r) => r.method === "DELETE" && r.path === "/api/v1/admin/browsers/0199f0a0-0000-7000-8000-000000000052")).toBe(true);
    await page.reload();
    await expect(page.getByRole("region", { name: "Signed in to this admin" }).getByText("2 browsers")).toBeVisible();
  });

  test("search narrows the list by person or computer", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/devices");
    await page.getByRole("searchbox", { name: "Search people or computers" }).fill("maria");
    await page.keyboard.press("Enter");
    await expect(page.getByText("Orchester on maria-mbp")).toBeVisible();
    await expect(page.getByText("Orchester on IVAN-PC")).toHaveCount(0);
    await page.goto("/admin/devices?q=nobody");
    await expect(page.getByText("No computer matches nobody")).toBeVisible();
  });
});
