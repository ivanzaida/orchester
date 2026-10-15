import { expect, test, type Page } from "@playwright/test";

import { MBP } from "../fake-api/accounts";

import { apiRequests, PASSWORD, resetFake, signInAs } from "./support";

const FULL = "mira.koval@example.com";
const NEW = "new.account@example.com";

async function openAccount(page: Page, email: string): Promise<void> {
  await signInAs(page, email, PASSWORD);
  await expect(page).toHaveURL(/\/account$/);
}

function section(page: Page, name: string) {
  return page.getByRole("region", { name });
}

test.describe("account", () => {
  test("a full account shows its invites, servers, devices and settings (H47EAc)", async ({ page }) => {
    await openAccount(page, FULL);
    await expect(page.getByRole("banner").getByText("Mira Koval")).toBeVisible();
    await expect(page.getByRole("navigation").getByRole("link", { name: "Invites · 2" })).toBeVisible();
    await expect(section(page, "Get started").getByText("Orchester is signed in on 2 of your computers")).toBeVisible();
    await expect(section(page, "Invites").getByText("2 waiting")).toBeVisible();
    await expect(section(page, "Invites").getByText("Tomas Berg invited you to a project on Northwind studio.")).toBeVisible();
    await expect(section(page, "Invites").getByText("Project · Member")).toBeVisible();
    await expect(section(page, "Your servers").getByText("2 servers")).toBeVisible();
    await expect(section(page, "Your servers").getByText("You own it")).toBeVisible();
    await expect(section(page, "Devices").getByText("3 signed in")).toBeVisible();
    await expect(section(page, "Devices").getByText("Orchester on mira-desktop")).toBeVisible();
    await expect(section(page, "Devices").getByText("Orchester app · macOS")).toBeVisible();
    await expect(section(page, "Account").getByText("Confirmed")).toBeVisible();
  });

  test("downloads are shown unavailable, with the reason, until they are published", async ({ page }) => {
    await openAccount(page, FULL);
    await expect(page.getByRole("button", { name: "Download for Windows" })).toBeDisabled();
    await expect(page.getByText("Downloads are not published yet.")).toBeVisible();
  });

  test("a new account gets the full Get started and empty sections (Z4Q3F)", async ({ page }) => {
    await openAccount(page, NEW);
    await expect(page.getByText("Get Orchester")).toBeVisible();
    await expect(page.getByText(`Settings › Orchester, with ${NEW}.`)).toBeVisible();
    await expect(page.getByText("No invites yet")).toBeVisible();
    await expect(page.getByText("No servers yet")).toBeVisible();
    await expect(section(page, "Devices").getByText("1 signed in")).toBeVisible();
  });
});

test.describe("account actions", () => {
  test("another device signs out at once and its row says so (wcFhu)", async ({ page }) => {
    await openAccount(page, FULL);
    const devices = section(page, "Devices");
    await devices.locator("form", { has: page.locator(`input[value="${MBP}"]`) }).getByRole("button").click();
    await expect(devices.getByText("Orchester on mira-mbp is signed out")).toBeVisible();
    await expect(devices.getByText("2 signed in")).toBeVisible();
    await expect(page.getByRole("navigation").getByRole("link", { name: "Devices" })).toHaveAttribute("aria-current", "location");
    expect((await apiRequests()).at(-1)).toMatchObject({ method: "DELETE", path: `/api/v1/devices/${MBP}` });
  });

  test("a wrong current password is said at its field, and the fields come back empty (ZyAsL)", async ({ page }) => {
    await openAccount(page, FULL);
    await page.getByRole("button", { name: "Change password" }).click();
    await page.getByLabel("Current password").fill("not the one");
    await page.getByLabel("New password", { exact: true }).fill("a long enough phrase");
    await page.getByLabel("Repeat new password").fill("a long enough phrase");
    await page.getByRole("button", { name: "Change password" }).click();
    await expect(page.getByText("The current password is not right.")).toBeVisible();
    await expect(page.getByLabel("Current password")).toHaveValue("");
    await expect(page.getByLabel("New password", { exact: true })).toHaveValue("");
    await expect(page.getByRole("navigation").getByRole("link", { name: "Account" })).toHaveAttribute("aria-current", "location");
  });

  test("a changed password closes the form and signs out the other devices", async ({ page }) => {
    await openAccount(page, FULL);
    await page.getByRole("button", { name: "Change password" }).click();
    await page.getByLabel("Current password").fill(PASSWORD);
    await page.getByLabel("New password", { exact: true }).fill(PASSWORD);
    await page.getByLabel("Repeat new password").fill(PASSWORD);
    await page.getByRole("button", { name: "Change password" }).click();
    await expect(page.getByText("Your password is changed.", { exact: false })).toBeVisible();
    await expect(section(page, "Devices").getByText("1 signed in")).toBeVisible();
  });


  test("the count stays right when the list reloads after a device was signed out", async ({ page }) => {
    await resetFake();
    await openAccount(page, FULL);
    const devices = section(page, "Devices");
    await devices.locator("form", { has: page.locator(`input[value="${MBP}"]`) }).getByRole("button").click();
    await expect(devices.getByText("2 signed in")).toBeVisible();
    await page.getByRole("button", { name: "Change password" }).click();
    await page.getByLabel("Current password").fill(PASSWORD);
    await page.getByLabel("New password", { exact: true }).fill(PASSWORD);
    await page.getByLabel("Repeat new password").fill(PASSWORD);
    await page.getByRole("button", { name: "Change password" }).click();
    await expect(page.getByText("Your password is changed.", { exact: false })).toBeVisible();
    await expect(devices.getByText("1 signed in")).toBeVisible();
  });

  test("deleting the account asks for its password in place, then leaves for sign-in", async ({ page }) => {
    await resetFake();
    await openAccount(page, NEW);
    await page.getByRole("button", { name: "Delete account" }).click();
    await expect(page.getByText("Delete your Orchester account?")).toBeVisible();
    await expect(page.getByText(`the invites to ${NEW} are deleted at once`, { exact: false })).toBeVisible();
    await page.getByRole("button", { name: "Keep" }).click();
    await expect(page.getByText("Delete your Orchester account?")).toHaveCount(0);
    await page.getByRole("button", { name: "Delete account" }).click();
    await page.getByLabel("Password", { exact: true }).fill("not the password");
    await page.getByRole("button", { name: "Delete account" }).click();
    await expect(page.getByText("The password is not right.")).toBeVisible();
    await expect(page.getByLabel("Password", { exact: true })).toHaveValue("");
    await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
    await page.getByRole("button", { name: "Delete account" }).click();
    await expect(page).toHaveURL(/\/signin$/);
    expect((await apiRequests()).at(-1)).toMatchObject({ method: "DELETE", path: "/api/v1/me", body: { password: "<redacted>" } });
    await signInAs(page, NEW, PASSWORD);
    await expect(page.getByText("The email or password is not right.", { exact: false })).toBeVisible();
    await resetFake();
  });
});

test.describe("account settings", () => {
  test("the display name saves only when it changed", async ({ page }) => {
    await openAccount(page, NEW);
    const save = page.getByRole("button", { name: "Save" });
    await expect(save).toBeDisabled();
    await page.getByLabel("Display name").fill("Nova Lind-Berg");
    await save.click();
    await expect(page.getByRole("banner").getByText("Nova Lind-Berg")).toBeVisible();
    await expect(save).toBeDisabled();
  });

  test("declining an invite removes it", async ({ page }) => {
    await openAccount(page, FULL);
    const invites = section(page, "Invites");
    await invites.getByRole("button", { name: "Decline" }).first().click();
    await expect(invites.getByText("1 waiting")).toBeVisible();
  });

  test("on a phone the nav is gone and the account sits under the brand (F9QYD)", async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await openAccount(page, FULL);
    await expect(page.getByRole("navigation")).toBeHidden();
    const brand = await page.getByRole("banner").getByText("Orchester", { exact: true }).boundingBox();
    const name = await page.getByRole("banner").getByText("Mira Koval").boundingBox();
    expect((name?.y ?? 0) > (brand?.y ?? 0) + (brand?.height ?? 0)).toBe(true);
  });
});
