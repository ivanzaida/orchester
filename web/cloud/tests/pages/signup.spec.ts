import { expect, test } from "@playwright/test";

import { apiRequests } from "./support";

test.describe("sign up", () => {
  test("checks the fields in the browser before sending", async ({ page }) => {
    await page.goto("/signup");
    await page.getByLabel("Email").fill("mira.koval@example");
    await page.getByLabel("Display name").fill("Mira Koval");
    await page.getByLabel("Password", { exact: true }).fill("12345678");
    const before = (await apiRequests()).length;
    await page.getByRole("button", { name: "Create account" }).click();
    await expect(page.getByText("Enter a full email address, like name@example.com.")).toBeVisible();
    await expect(page.getByText("Use at least 12 characters; this one has 8.")).toBeVisible();
    // The rule hint gives way to the error, and nothing went to the cloud.
    await expect(page.getByText("12 to 256 characters.", { exact: false })).toHaveCount(0);
    await expect(page.getByLabel("Email")).toHaveAttribute("aria-invalid", "true");
    expect((await apiRequests()).length).toBe(before);
  });

  test("sends the account and shows Check your email with the cooldown", async ({ page }) => {
    await page.goto("/signup");
    await page.getByLabel("Email").fill(" New.Person@Example.com ");
    await page.getByLabel("Display name").fill("New Person");
    await page.getByLabel("Password", { exact: true }).fill("a long enough phrase");
    await page.getByRole("button", { name: "Create account" }).click();
    await expect(page).toHaveURL(/\/signup\/sent$/);
    await expect(page.getByRole("heading", { name: "Check your email" })).toBeVisible();
    await expect(page.getByText("new.person@example.com")).toBeVisible();
    await expect(page.getByRole("button", { name: "Resend email" })).toBeDisabled();
    await expect(page.getByText(/You can resend in 0:[0-5]\d\./)).toBeVisible();
    const sent = (await apiRequests()).filter((request) => request.path === "/api/v1/auth/signup").at(-1);
    expect(sent?.body).toEqual({ email: "new.person@example.com", password: "<redacted>", display_name: "New Person" });
    await page.getByRole("link", { name: "Use another email" }).click();
    await expect(page).toHaveURL(/\/signup$/);
    await expect(page.getByLabel("Email")).toHaveValue("");
  });

  test("says when the cloud does not answer or refuses more emails", async ({ page }) => {
    await page.goto("/signup");
    await page.getByLabel("Display name").fill("Someone");
    await page.getByLabel("Password", { exact: true }).fill("a long enough phrase");
    await page.getByLabel("Email").fill("down@example.com");
    await page.getByRole("button", { name: "Create account" }).click();
    await expect(page.getByText("Orchester Cloud is not answering right now. Try again in a minute.")).toBeVisible();
    await expect(page.getByLabel("Email")).toHaveValue("down@example.com");
    await page.getByLabel("Email").fill("limited@example.com");
    await page.getByRole("button", { name: "Create account" }).click();
    await expect(page.getByText(/Too many emails were sent\. Try again in 38 min, at \d\d:\d\d\./)).toBeVisible();
  });

  test("shows the phone layout with the card at full width", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 900 });
    await page.goto("/signup");
    const card = await page.locator("section").boundingBox();
    expect(card?.x).toBe(16);
    expect(card?.width).toBe(358);
  });
});

test.describe("check your email", () => {
  test("without a sign-up first leads back to the form", async ({ page }) => {
    await page.goto("/signup/sent");
    await expect(page).toHaveURL(/\/signup$/);
  });
});
