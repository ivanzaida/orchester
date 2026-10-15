import { expect, test } from "@playwright/test";

import { apiRequests, EMAIL, expectNoApiAddress, PASSWORD, signInAs, watchBrowserRequests } from "./support";

test.describe("sign in", () => {
  test("a wrong password says so once, keeps the email and clears the password", async ({ page }) => {
    await signInAs(page, EMAIL, "not the right one");
    await expect(page.getByText("The email or password is not right. Check both, or reset your password.")).toBeVisible();
    await expect(page.getByLabel("Email")).toHaveValue(EMAIL);
    await expect(page.getByLabel("Password", { exact: true })).toHaveValue("");
    await expect(page.getByLabel("Email")).not.toHaveAttribute("aria-invalid", "true");
  });

  test("an unconfirmed email offers to resend the confirmation", async ({ page }) => {
    await signInAs(page, "unconfirmed@example.com", PASSWORD);
    await expect(page.getByText("Confirm your email first")).toBeVisible();
    await expect(page.getByText("We sent a link to unconfirmed@example.com when you signed up.", { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Resend confirmation" }).click();
    await expect(page).toHaveURL(/\/signup\/sent$/);
    await expect(page.getByText("unconfirmed@example.com")).toBeVisible();
    await expect(page.getByText(/Sent again at \d\d:\d\d\./)).toBeVisible();
  });

  test("too many attempts stops the button and says until when", async ({ page }) => {
    await signInAs(page, "limited@example.com", PASSWORD);
    await expect(page.getByText(/Too many sign-in attempts\. Try again in 15 min, at \d\d:\d\d\./)).toBeVisible();
    // At once, together with the error: not a second later at the countdown's first tick.
    expect(await page.getByRole("button", { name: "Sign in" }).isDisabled()).toBe(true);
    await expect(page.getByRole("button", { name: "Sign in" })).toBeDisabled();
    await expect(page.getByText(/Sign in unavailable until \d\d:\d\d\./)).toBeVisible();
  });

  test("signs in without the browser ever holding the token or calling the API", async ({ page, context }) => {
    const urls = watchBrowserRequests(page);
    await signInAs(page);
    await expect(page).toHaveURL(/\/account$/);
    await expect(page.getByRole("heading", { name: "Get started" })).toBeVisible();
    await expect(page.getByText(EMAIL).first()).toBeVisible();

    const signIn = (await apiRequests()).filter((request) => request.path === "/api/v1/auth/sessions").at(-1);
    expect(signIn?.body?.device).toEqual({ name: "Edge on Windows", platform: "web" });
    const meCall = (await apiRequests()).filter((request) => request.path === "/api/v1/me").at(-1);
    const token = meCall?.authorization?.replace("Bearer ", "") ?? "";
    expect(token).toMatch(/^ocd_fake_web_token_/);
    expect(meCall?.cookie).toBeNull();

    const cookies = await context.cookies();
    const session = cookies.find((cookie) => cookie.name === "__Host-orchester_session");
    expect(session).toMatchObject({ httpOnly: true, secure: true, sameSite: "Lax", path: "/" });
    expect(session?.domain).toBe("localhost");
    for (const cookie of cookies) {
      expect(cookie.value).not.toContain(token);
    }
    expect(await page.content()).not.toContain(token);
    expect(await page.evaluate(() => document.cookie)).toBe("");
    expectNoApiAddress(urls);

    await page.getByRole("banner").getByRole("button", { name: "Sign out" }).click();
    await expect(page).toHaveURL(/\/signin$/);
    expect((await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_session")).toBe(false);
    expect((await apiRequests()).at(-1)).toMatchObject({ method: "DELETE", path: "/api/v1/auth/sessions/current" });
  });

  test("the home page without a session is the sign-in page", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveURL(/\/signin$/);
  });

  test("shows the light design when the browser asks for it", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto("/signin");
    const background = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(background).toBe("rgb(228, 228, 232)");
  });
});
