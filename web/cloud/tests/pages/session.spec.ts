import { expect, test, type BrowserContext } from "@playwright/test";

import { apiRequests, revokeAtCloud, signInAs } from "./support";

const NEW = "new.account@example.com";

async function hasSession(context: BrowserContext): Promise<boolean> {
  return (await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_session");
}

test.describe("a session the cloud ended", () => {
  test("reloading the account forgets it and asks to sign in", async ({ page, context }) => {
    await signInAs(page, NEW);
    await expect(page).toHaveURL(/\/account$/);
    await revokeAtCloud();
    await page.reload();
    await expect(page).toHaveURL(/\/signin$/);
    expect(await hasSession(context)).toBe(false);
  });

  test("an action on the page forgets it and asks to sign in, instead of saying the cloud is not answering", async ({ page, context }) => {
    await signInAs(page, NEW);
    await expect(page).toHaveURL(/\/account$/);
    await revokeAtCloud();
    await page.getByLabel("Display name").fill("Someone Else");
    await page.getByRole("button", { name: "Save" }).click();
    await expect(page).toHaveURL(/\/signin$/);
    await expect(page.getByText("Orchester Cloud is not answering", { exact: false })).toHaveCount(0);
    expect(await hasSession(context)).toBe(false);
  });

  test("signing in again ends the earlier session at the cloud first", async ({ page }) => {
    await signInAs(page, NEW);
    await expect(page).toHaveURL(/\/account$/);
    const first = (await apiRequests()).filter((r) => r.path === "/api/v1/me").at(-1)?.authorization;
    await signInAs(page, NEW);
    await expect(page).toHaveURL(/\/account$/);
    const ended = (await apiRequests()).filter((r) => r.method === "DELETE" && r.path === "/api/v1/auth/sessions/current");
    expect(ended.map((r) => r.authorization)).toContain(first);
  });
});
