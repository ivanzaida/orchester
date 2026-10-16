import { expect, test } from "@playwright/test";

import { ADMIN, authorizePosts, apiRequests, reset, signIn } from "./support";

test.beforeEach(async () => {
  await reset();
});

test.describe("settings", () => {
  test("join approval and cloud sign-ins are switches", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/settings");
    const approval = page.getByRole("switch", { name: "Join approval" });
    await expect(approval).toHaveAttribute("aria-checked", "true");
    await approval.click();
    await expect(page.getByRole("switch", { name: "Join approval" })).toHaveAttribute("aria-checked", "false");
    await page.getByRole("switch", { name: "Sign-ins through the cloud" }).click();
    await expect(page.getByText("Nobody joins through Orchester Cloud now")).toBeVisible();
    const patches = (await apiRequests()).filter((r) => r.method === "PATCH").map((r) => r.body);
    expect(patches).toEqual([{ join_approval: false }, { accepting: false }]);
  });

  test("pins another key in place", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/settings");
    await page.getByRole("button", { name: "Pin another key…" }).click();
    await page.getByLabel("New root key").fill("new-root-key");
    await page.getByRole("button", { name: "Pin this key" }).click();
    await expect(page.getByText("new-root-key")).toBeVisible();
    expect((await apiRequests()).find((r) => r.method === "PATCH")?.body).toEqual({ pin: "new-root-key" });
  });

  test("unlinks after a question in place, then offers to link again through the cloud", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/settings");
    await page.getByRole("button", { name: "Unlink…" }).click();
    await page.getByRole("group", { name: "Unlink Kontur team from Orchester Cloud?" }).getByRole("button", { name: "Unlink" }).click();
    await expect(page.getByText("Not linked", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Link to Orchester Cloud" }).click();
    await expect(page).toHaveURL("http://127.0.0.1:4220/authorize/start");
    expect((await authorizePosts()).at(-1)?.purpose).toBe("link");
    await page.getByRole("button", { name: "Continue as ivan@example.com" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/settings?link=done`);
    await expect(page.getByText("Kontur team is linked to Orchester Cloud.")).toBeVisible();
    await expect(page.getByText("Linked", { exact: true })).toBeVisible();
  });

  test("an out-of-date key list is said", async ({ page }) => {
    await reset("stale");
    await signIn(page);
    await page.goto("/admin/settings");
    await expect(page.getByText("Orchester Cloud's keys here are out of date")).toBeVisible();
  });

  test("shows the certificate's check code", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/settings");
    await expect(page.getByText("3F7A-92C1-KD8M-QX4T")).toBeVisible();
    await expect(page.getByText("Made by this server")).toBeVisible();
  });
});
