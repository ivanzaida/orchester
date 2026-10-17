import { expect, test } from "@playwright/test";

import { ADMIN, CLOUD, apiRequests, deliveries, reset, signIn } from "./support";

test.beforeEach(async () => {
  await reset();
});

test.describe("invites", () => {
  test("an email invite is made by the server and delivered to the cloud from the admin's side", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByLabel("Email", { exact: true }).fill("New.Person@Example.com");
    await page.getByLabel("Role").first().selectOption("owner");
    await page.getByRole("button", { name: "Send invite" }).click();
    await expect(page.getByText("Invite sent to new.person@example.com")).toBeVisible();
    await expect(page.getByLabel("Email", { exact: true })).toHaveValue("");
    const made = (await apiRequests()).find((r) => r.method === "POST" && r.path === "/api/v1/invites");
    expect(made?.body).toMatchObject({ kind: "project", delivery: "cloud", email: "new.person@example.com", role: "owner" });
    expect(await deliveries()).toContainEqual({ how: "server", email: "new.person@example.com" });
  });

  test("when this host can't reach the cloud, the browser sends the signed invite", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByLabel("Email", { exact: true }).fill("offline.person@example.com");
    await page.getByRole("button", { name: "Send invite" }).click();
    await expect(page.getByText("Kontur team can't reach Orchester Cloud")).toBeVisible();
    await page.getByRole("button", { name: "Send through this browser" }).click();
    await expect(page).toHaveURL(`${CLOUD}/invites/deliver`);
    await expect(page.getByText("Invite delivered.")).toBeVisible();
    expect(await deliveries()).toContainEqual({ how: "browser", email: "offline.person@example.com" });
    // Security review 1, minor 2: the form names no address to come back to; the cloud leads to the one it registered.
    await expect(page.locator("[data-fields]")).toHaveAttribute("data-fields", "invite");
    await page.getByRole("link", { name: "Back" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin`);
  });

  test("an admin without a two-factor sign-in is told where to turn it on, and the address stays", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByLabel("Email", { exact: true }).fill("no-mfa.person@example.com");
    await page.getByRole("button", { name: "Send invite" }).click();
    await expect(page.getByText("Two-factor authentication needed")).toBeVisible();
    await expect(page.getByText("Turn on two-factor authentication in your Orchester Cloud account to send invites.", { exact: false })).toBeVisible();
    await expect(page.getByRole("link", { name: "Open Orchester Cloud security" })).toHaveAttribute("href", `${CLOUD}/account#security`);
    await expect(page.getByLabel("Email", { exact: true })).toHaveValue("no-mfa.person@example.com");
    expect((await deliveries()).some((delivery) => delivery.email === "no-mfa.person@example.com")).toBe(false);
  });

  test("making a code without a two-factor sign-in says how to get one", async ({ page }) => {
    await reset("no-mfa");
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByRole("button", { name: "Make a code" }).click();
    await expect(page.getByText("Making invites, codes too, needs two-factor authentication", { exact: false })).toBeVisible();
  });

  test("the daily limit stops the button and says until when", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByLabel("Email", { exact: true }).fill("limit@example.com");
    await page.getByRole("button", { name: "Send invite" }).click();
    await expect(page.getByText("You've sent 30 invites today")).toBeVisible();
    await expect(page.getByText(/Try again at \d\d:\d\d, or invite with a code\./)).toBeVisible();
    await expect(page.getByRole("button", { name: "Send invite" })).toBeDisabled();
  });

});

test.describe("invite codes and the list", () => {
  test("a code is shown once, in the page that made it only", async ({ page, context }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByRole("button", { name: "Make a code" }).click();
    await expect(page.getByText("Invite code for Kontur · member")).toBeVisible();
    await expect(page.getByText("K7QM-3XRD")).toBeVisible();
    await expect(page.getByText("3F7A-92C1-KD8M-QX4T")).toBeVisible();
    expect(page.url()).not.toContain("K7QM");
    for (const cookie of await context.cookies()) {
      expect(cookie.value).not.toContain("K7QM");
    }
    await page.getByRole("button", { name: "Done, I've given it" }).click();
    await expect(page.getByText("K7QM-3XRD")).toHaveCount(0);
    await page.reload();
    await expect(page.getByText("K7QM-3XRD")).toHaveCount(0);
  });

  test("revoking asks in place, and an email invite is revoked at the cloud too", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByRole("button", { name: "Revoke" }).nth(1).click();
    const question = page.getByRole("group", { name: "Revoke the invite to dana@example.com?" });
    await expect(question).toBeVisible();
    await question.getByRole("button", { name: "Revoke" }).click();
    await expect(page.getByText("The invite is revoked")).toBeVisible();
    expect(await deliveries()).toContainEqual({ how: "revocation", email: "dana@example.com" });
  });

  test("ended invites keep no address and offer nothing", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await expect(page.getByText("Revoked", { exact: true })).toBeVisible();
    await expect(page.getByText("Maria Lind joined", { exact: false })).toBeVisible();
    await expect(page.getByText("lena@example.com")).toHaveCount(0);
  });

  test("without a link to the cloud, email invites are said to be unavailable", async ({ page }) => {
    await reset("unlinked");
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await expect(page.getByText("Email invites need Orchester Cloud")).toBeVisible();
    await expect(page.getByRole("button", { name: "Send invite" })).toHaveCount(0);
    await expect(page.getByRole("button", { name: "Make a code" })).toBeVisible();
  });
});
