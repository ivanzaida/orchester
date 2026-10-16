import { expect, test } from "@playwright/test";

import { ADMIN, apiRequests, reset } from "./support";

test.beforeEach(async () => {
  await reset("setup");
});

test.describe("first setup", () => {
  test("a server without an admin sends every page to the setup", async ({ page }) => {
    for (const path of ["/admin", "/admin/signin"]) {
      await page.goto(path);
      await expect(page).toHaveURL(`${ADMIN}/admin/setup`);
    }
    await expect(page.getByRole("heading", { name: "Set up this Orchester server" })).toBeVisible();
    await expect(page.getByRole("list", { name: "Setup steps" })).toBeVisible();
  });

  test("a wrong setup code says so under the field", async ({ page }) => {
    await page.goto("/admin/setup");
    await page.getByLabel("Setup code").fill("Q4ZD-8M2X");
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByText("That isn't the setup code, or it has expired.", { exact: false })).toBeVisible();
  });

  test("the code, the account, then the cloud or not now, then ready", async ({ page, context }) => {
    await page.goto("/admin/setup");
    await page.getByLabel("Setup code").fill("q4zd 8m2k");
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/setup/account`);
    const setup = (await context.cookies()).find((cookie) => cookie.name === "__Host-orchester_admin_setup");
    expect(setup).toMatchObject({ httpOnly: true, secure: true, sameSite: "Lax" });
    expect(setup?.value.toUpperCase()).not.toContain("Q4ZD");
    expect(await page.content()).not.toContain("q4zd");

    await page.getByLabel("Server name").fill("Kontur team");
    await page.getByLabel("Username").fill("taken");
    await page.getByLabel("Display name").fill("Ivan Zaida");
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page.getByText("This username is taken. Choose another.")).toBeVisible();
    await page.getByLabel("Username").fill("ivan");
    await page.getByRole("button", { name: "Continue" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/setup/cloud`);
    expect((await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_admin_setup")).toBe(false);
    expect((await apiRequests()).find((r) => r.path === "/api/v1/server/settings" && r.method === "PATCH")?.body).toEqual({ name: "Kontur team" });

    await expect(page.getByRole("heading", { name: "Link Kontur team to Orchester Cloud" })).toBeVisible();
    await page.getByRole("link", { name: "Not now" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/setup/done`);
    await expect(page.getByRole("heading", { name: "Kontur team is ready" })).toBeVisible();
    await expect(page.getByText("Not linked to Orchester Cloud")).toBeVisible();
    await expect(page.getByText("ivan · admin")).toBeVisible();
  });

  test("linking at setup goes to the cloud and back to ready", async ({ page }) => {
    await page.goto("/admin/setup");
    await page.getByLabel("Setup code").fill("Q4ZD-8M2K");
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByLabel("Username").fill("ivan");
    await page.getByLabel("Display name").fill("Ivan Zaida");
    await page.getByRole("button", { name: "Continue" }).click();
    await page.getByRole("button", { name: "Link to Orchester Cloud" }).click();
    await expect(page).toHaveURL(/127\.0\.0\.1:4220\/authorize\?purpose=link&statement=fake-statement/);
    await page.getByRole("button", { name: "Continue as ivan@example.com" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/setup/done`);
    await expect(page.getByText("Linked by Ivan Zaida")).toBeVisible();
    const link = (await apiRequests()).filter((r) => r.path === "/api/v1/server/cloud" && r.method === "PATCH").at(-1);
    expect(link?.body).toMatchObject({ link: { root_key: "fake-root-key", key_set: "fake-key-set" } });
  });

  test("a set-up server takes no setup code", async ({ page }) => {
    await reset("full");
    await page.goto("/admin/setup");
    await expect(page.getByRole("heading", { name: "Kontur team is already set up" })).toBeVisible();
    await page.goto("/admin/setup/account");
    await expect(page).toHaveURL(`${ADMIN}/admin/setup`);
  });
});
