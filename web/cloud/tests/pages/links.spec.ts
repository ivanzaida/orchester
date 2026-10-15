import { expect, test } from "@playwright/test";

import { apiRequests, EMAIL } from "./support";

test.describe("confirmation link", () => {
  test("opening it confirms nothing; the press does", async ({ page }) => {
    await page.goto("/confirm/confirm-ok");
    await expect(page.getByRole("heading", { name: "Confirm your email" })).toBeVisible();
    await expect(page.getByText(EMAIL)).toBeVisible();
    const paths = (await apiRequests()).map((request) => request.path);
    expect(paths.at(-1)).toBe("/api/v1/auth/links/inspect");
    await page.getByRole("button", { name: "Confirm email" }).click();
    await expect(page.getByRole("heading", { name: "Your email is confirmed" })).toBeVisible();
    await expect(page.getByText(EMAIL)).toBeVisible();
    expect((await apiRequests()).at(-1)).toMatchObject({ path: "/api/v1/auth/confirm", body: { link_token: "confirm-ok" } });
    await page.getByRole("link", { name: "Sign in on the web" }).click();
    await expect(page).toHaveURL(/\/signin$/);
  });

  test("an expired or used link offers a new one", async ({ page }) => {
    await page.goto("/confirm/used");
    await expect(page.getByRole("heading", { name: "This link no longer works" })).toBeVisible();
    await page.getByLabel("Email").fill(EMAIL);
    await page.getByRole("button", { name: "Send a new link" }).click();
    await expect(page).toHaveURL(/\/signup\/sent$/);
    expect((await apiRequests()).at(-1)).toMatchObject({ path: "/api/v1/auth/confirm/resend", body: { email: EMAIL } });
  });

  test("without a token is no page", async ({ page }) => {
    const response = await page.goto("/confirm");
    expect(response?.status()).toBe(404);
    await expect(page.getByRole("heading", { name: "This page does not exist" })).toBeVisible();
  });

  test("a cloud that fails says so instead of a page that pretends", async ({ page }) => {
    await page.goto("/confirm/inspect-down");
    await expect(page.getByRole("heading", { name: "Orchester Cloud is not answering" })).toBeVisible();
  });
});

test.describe("forgot and reset", () => {
  test("forgot password answers the same for any address", async ({ page }) => {
    await page.goto("/forgot");
    await page.getByLabel("Email").fill("nobody@example.com");
    await page.getByRole("button", { name: "Send reset link" }).click();
    await expect(page).toHaveURL(/\/forgot\/sent$/);
    await expect(page.getByText("If this email has an Orchester account, we sent it a link to reset the password:")).toBeVisible();
    await expect(page.getByText("nobody@example.com")).toBeVisible();
  });

  test("the reset form checks both fields, then sets the password", async ({ page }) => {
    await page.goto("/reset/reset-ok");
    await expect(page.getByText(`For the Orchester account ${EMAIL}.`)).toBeVisible();
    await page.getByLabel("New password", { exact: true }).fill("123456789");
    await page.getByLabel("Repeat new password").fill("123456789012");
    await page.getByRole("button", { name: "Set password" }).click();
    await expect(page.getByText("Use at least 12 characters; this one has 9.")).toBeVisible();
    await expect(page.getByText("The two passwords are different.")).toBeVisible();
    await page.getByLabel("New password", { exact: true }).fill("a long enough phrase");
    await page.getByLabel("Repeat new password").fill("a long enough phrase");
    await page.getByRole("button", { name: "Set password" }).click();
    await expect(page.getByRole("heading", { name: "Your password is changed" })).toBeVisible();
    const finish = (await apiRequests()).filter((request) => request.path === "/api/v1/auth/reset/finish");
    expect(finish).toHaveLength(1);
    expect(finish[0]?.body).toEqual({ link_token: "reset-ok", new_password: "<redacted>" });
    // Nothing looked at the link again after it was used.
    expect((await apiRequests()).at(-1)?.path).toBe("/api/v1/auth/reset/finish");
  });

  test("an expired reset link sends a new one through Forgot password", async ({ page }) => {
    await page.goto("/reset/used");
    await expect(page.getByRole("heading", { name: "This reset link no longer works" })).toBeVisible();
    await page.getByLabel("Email").fill(EMAIL);
    await page.getByRole("button", { name: "Send a new link" }).click();
    await expect(page).toHaveURL(/\/forgot\/sent$/);
  });
});
