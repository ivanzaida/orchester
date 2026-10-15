import { expect, test, type BrowserContext } from "@playwright/test";

import { SIGNUP_FLOW_COOKIE } from "@/lib/server/cookie-options";
import { deriveKey } from "@/lib/server/seal";
import { signFlow } from "@/lib/server/session";

import { TEST_ENV } from "../../playwright.config";

/**
 * *Check your email* after its 60 s: the test signs the page's cookie with the test secret, as if the email had gone
 * out a minute ago, instead of waiting.
 */
async function sentAMinuteAgo(context: BrowserContext, email: string): Promise<void> {
  const key = deriveKey(new Uint8Array(Buffer.from(TEST_ENV.ORCHESTER_CLOUD_WEB_SECRET, "base64")), "flow");
  const value = signFlow(key, { email, sentAt: Date.now() - 61_000, resent: false }, Date.now(), 3600);
  await context.addCookies([
    { name: SIGNUP_FLOW_COOKIE, value, domain: "localhost", path: "/", secure: true, httpOnly: true, sameSite: "Lax" },
  ]);
}

test.describe("resend", () => {
  test("after the cooldown sends again and starts the cooldown over", async ({ page, context }) => {
    await sentAMinuteAgo(context, "mira.koval@example.com");
    await page.goto("/signup/sent");
    const resend = page.getByRole("button", { name: "Resend email" });
    await expect(resend).toBeEnabled();
    await resend.click();
    await expect(page.getByText(/Sent again at \d\d:\d\d\. It can take a minute to arrive\./)).toBeVisible();
    await expect(page.getByText(/You can resend in 0:5\d\./)).toBeVisible();
    await expect(page.getByRole("button", { name: "Resend email" })).toBeDisabled();
  });

  test("at the cloud's limit says until when", async ({ page, context }) => {
    await sentAMinuteAgo(context, "resend-limit@example.com");
    await page.goto("/signup/sent");
    await page.getByRole("button", { name: "Resend email" }).click();
    await expect(page.getByText(/Resend unavailable until \d\d:\d\d\. We send at most 5 emails an hour to one address\./)).toBeVisible();
    await expect(page.getByRole("button", { name: "Resend email" })).toBeDisabled();
    await expect(page.getByText(/Sent again/)).toHaveCount(0);
  });
});
