import { expect, test } from "@playwright/test";

import { ADMIN, apiRequests, reset, signIn } from "./support";

test.beforeEach(async () => {
  await reset();
});

test.describe("members", () => {
  test("a role changes when the select does", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur");
    await page.getByRole("combobox", { name: "Role of Oleh Petrenko" }).selectOption("owner");
    await expect.poll(async () => (await apiRequests()).find((r) => r.method === "PATCH")?.body).toEqual({ role: "owner" });
    await expect(page.getByRole("combobox", { name: "Role of Oleh Petrenko" })).toHaveValue("owner");
  });

  test("removing asks in place; Keep keeps, Remove removes", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur");
    await page.getByRole("button", { name: "Remove" }).nth(1).click();
    const question = page.getByRole("group", { name: "Remove Oleh Petrenko from Kontur?" });
    await expect(question).toBeVisible();
    await question.getByRole("button", { name: "Keep" }).click();
    await expect(question).toHaveCount(0);
    await page.getByRole("button", { name: "Remove" }).nth(1).click();
    await page.getByRole("group", { name: "Remove Oleh Petrenko from Kontur?" }).getByRole("button", { name: "Remove" }).click();
    await expect(page.getByText("Oleh Petrenko is removed from Kontur")).toBeVisible();
    expect((await apiRequests()).some((r) => r.method === "DELETE" && r.path.endsWith("/members/0199f0a0-0000-7000-8000-000000000003"))).toBe(true);
  });

  test("the only owner can't leave or become a member", async ({ page }) => {
    await signIn(page, "OWNR-0001");
    await page.goto("/admin/projects/mobile-app");
    await page.getByRole("button", { name: "Leave" }).click();
    await page.getByRole("group", { name: "Leave Mobile app?" }).getByRole("button", { name: "Leave" }).click();
    await expect(page.getByText("Mobile app needs an owner")).toBeVisible();
    await expect(page.getByText("You're its only owner, so you can't leave or become a member.", { exact: false })).toBeVisible();
  });

  test("leaving a project goes back to the projects", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur");
    await page.getByRole("button", { name: "Leave" }).click();
    await page.getByRole("group", { name: "Leave Kontur?" }).getByRole("button", { name: "Leave" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin`);
  });
});
