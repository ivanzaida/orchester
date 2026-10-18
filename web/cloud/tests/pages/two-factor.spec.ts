import { expect, test, type Page } from "@playwright/test";

import { apiRequests, PASSWORD, resetFake, signInAs, twoFactorElsewhere } from "./support";

const FULL = "mira.koval@example.com";
const TWO_FACTOR = "two.factor@example.com";
const APP_CODE = "123456";
const RECOVERY_CODE = "abcde-fghjk";

function security(page: Page) {
  return page.getByRole("region", { name: "Security" });
}

test.describe("two-factor sign-in", () => {
  test("the password step asks for a code, which a recovery code can be", async ({ page, context }) => {
    await resetFake();
    await signInAs(page, TWO_FACTOR, PASSWORD);
    await expect(page).toHaveURL(/\/signin\/code$/);
    await expect(page.getByText(`Two-factor authentication is on for ${TWO_FACTOR}.`, { exact: false })).toBeVisible();
    expect((await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_session")).toBe(false);
    const challenge = (await context.cookies()).find((cookie) => cookie.name === "__Host-orchester_challenge");
    expect(challenge).toMatchObject({ httpOnly: true, secure: true, sameSite: "Lax" });
    expect(challenge?.value).not.toContain("orcm_");
    await page.getByLabel("Code").fill("000000");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText("This code is not right, or it was used already.", { exact: false })).toBeVisible();
    await expect(page.getByLabel("Code")).toHaveValue("");
    await page.getByLabel("Code").fill(RECOVERY_CODE);
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page).toHaveURL(/\/account$/);
    await expect(security(page).getByText("On", { exact: true })).toBeVisible();
    await expect(security(page).getByText("9 of 10 left.", { exact: false })).toBeVisible();
    await expect(security(page).getByText("This browser signed in without a code.")).toHaveCount(0);
    expect((await apiRequests()).filter((r) => r.path === "/api/v1/auth/sessions/two-factor").at(-1)?.body).toEqual({
      challenge: "<redacted>",
      code: "<redacted>",
    });
  });

  test("the code page without a waiting sign-in says to sign in again", async ({ page }) => {
    await page.goto("/signin/code");
    await expect(page.getByRole("heading", { name: "Sign in again" })).toBeVisible();
    await page.getByRole("link", { name: "Sign in again" }).click();
    await expect(page).toHaveURL(/\/signin$/);
  });
});

test.describe("security", () => {
  test("turning it on shows the QR code, then the recovery codes once; turning it off asks for both", async ({ page }) => {
    await resetFake();
    await signInAs(page, FULL, PASSWORD);
    await expect(page).toHaveURL(/\/account$/);
    await expect(page.getByText("Accepting an invite needs two-factor authentication.", { exact: false })).toBeVisible();
    await expect(security(page).getByText("Off", { exact: true })).toBeVisible();
    await security(page).getByRole("button", { name: "Turn on" }).click();
    await security(page).getByLabel("Password", { exact: true }).fill("not the password");
    await security(page).getByRole("button", { name: "Continue" }).click();
    await expect(security(page).getByText("The password is not right.")).toBeVisible();
    await security(page).getByLabel("Password", { exact: true }).fill(PASSWORD);
    await security(page).getByRole("button", { name: "Continue" }).click();
    await expect(security(page).getByRole("img", { name: "QR code for your authenticator app" })).toBeVisible();
    await expect(security(page).getByText("JBSW Y3DP EHPK 3PXP", { exact: false })).toBeVisible();
    await security(page).getByLabel("Code from the app").fill("000000");
    await security(page).getByRole("button", { name: "Turn on" }).click();
    await expect(security(page).getByText("This code is not right.", { exact: false })).toBeVisible();
    await security(page).getByLabel("Code from the app").fill(APP_CODE);
    await security(page).getByRole("button", { name: "Turn on" }).click();
    await expect(security(page).getByText("Your recovery codes", { exact: true })).toBeVisible();
    for (const code of ["abcde-fghjk", "kmnpq-rstvw"]) {
      await expect(security(page).getByText(code, { exact: true })).toBeVisible();
    }
    await expect(security(page).getByRole("link", { name: "Download" })).toHaveAttribute("download", "orchester-recovery-codes.txt");
    await security(page).getByRole("button", { name: "Done" }).click();
    await expect(security(page).getByText("On", { exact: true })).toBeVisible();
    await expect(security(page).getByText("Your recovery codes", { exact: true })).toHaveCount(0);
    await expect(page.getByText("Accepting an invite needs two-factor authentication.", { exact: false })).toHaveCount(0);
    await security(page).getByRole("button", { name: "Turn off" }).click();
    await security(page).getByLabel("Password", { exact: true }).fill(PASSWORD);
    await security(page).getByLabel("Code from the app or a recovery code").fill("000000");
    await security(page).getByRole("button", { name: "Turn off" }).click();
    await expect(security(page).getByText("This code is not right, or it was used already.")).toBeVisible();
    await security(page).getByLabel("Password", { exact: true }).fill(PASSWORD);
    await security(page).getByLabel("Code from the app or a recovery code").fill(APP_CODE);
    await security(page).getByRole("button", { name: "Turn off" }).click();
    await expect(security(page).getByText("Off", { exact: true })).toBeVisible();
  });

  test("a browser signed in before it was turned on elsewhere verifies with a code", async ({ page }) => {
    await resetFake();
    await signInAs(page, FULL, PASSWORD);
    await expect(page).toHaveURL(/\/account$/);
    await twoFactorElsewhere(FULL);
    await page.reload();
    await expect(security(page).getByText("This browser signed in without a code.", { exact: false })).toBeVisible();
    await security(page).getByLabel("Code from the app or a recovery code").first().fill(APP_CODE);
    await security(page).getByRole("button", { name: "Verify" }).click();
    await expect(security(page).getByText("This browser signed in without a code.", { exact: false })).toHaveCount(0);
  });
});
