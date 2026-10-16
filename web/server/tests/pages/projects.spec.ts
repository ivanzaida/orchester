import { expect, test } from "@playwright/test";

import { ADMIN, apiRequests, reset, signIn } from "./support";

test.beforeEach(async () => {
  await reset();
});

test.describe("projects", () => {
  test("lists every project for an admin, with the waiting computer", async ({ page }) => {
    await signIn(page);
    await expect(page.getByRole("link", { name: /Kontur/ }).first()).toBeVisible();
    // The wide page; the phone's own copies are in the page but not shown (`mYBka`, design.spec.ts).
    await expect(page.getByText("A computer is waiting for approval").filter({ visible: true })).toBeVisible();
    await expect(page.getByText("Orchester on oleh-pc wants to join as Oleh Petrenko.", { exact: false })).toBeVisible();
    await expect(page.getByText("No repository hint yet")).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Kontur team" }).getByRole("link", { name: "People · 5" })).toBeVisible();
    const call = (await apiRequests()).find((r) => r.path === "/api/v1/admin/projects");
    expect(call).toMatchObject({ forwardedHost: "localhost:4200", forwardedProto: "http" });
  });

  test("an owner who is not an admin sees only their project and no admin pages", async ({ page }) => {
    await signIn(page, "OWNR-0001");
    await expect(page.getByRole("banner").getByText("Project owner · signed in with a code").first()).toBeVisible();
    await expect(page.getByText("Mobile app").filter({ visible: true })).toBeVisible();
    await expect(page.getByText("Billing")).toHaveCount(0);
    await expect(page.getByRole("link", { name: /People/ })).toHaveCount(0);
    await expect(page.getByRole("link", { name: "New project" })).toHaveCount(0);
    await page.goto("/admin/people");
    await expect(page.getByText("People is for admins")).toBeVisible();
  });

  test("no projects yet offers to make one", async ({ page }) => {
    await reset("empty");
    await signIn(page);
    await expect(page.getByText("No projects yet")).toBeVisible();
  });
});

test.describe("new project", () => {
  test("the slug follows the name, and the server's names and taken slugs are refused", async ({ page }) => {
    await signIn(page);
    await page.getByRole("link", { name: "New project" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/projects/new`);
    await page.getByLabel("Name").fill("API Gateway");
    await expect(page.getByLabel("Slug")).toHaveValue("api-gateway");
    await page.getByLabel("Slug").fill("api");
    await page.getByRole("button", { name: "Create project" }).click();
    await expect(page.getByText("api is kept for the server itself. Choose another slug.")).toBeVisible();
    await page.getByLabel("Slug").fill("kontur");
    await page.getByRole("button", { name: "Create project" }).click();
    await expect(page.getByText("Another project on Kontur team has this address. Choose another slug.")).toBeVisible();
    await page.getByLabel("Slug").fill("API Gateway");
    await page.getByRole("button", { name: "Create project" }).click();
    await expect(page.getByText("Use small letters, digits and hyphens, like api-gateway.")).toBeVisible();
  });

  test("creates a project and offers to invite people", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/new");
    await page.getByLabel("Name").fill("API gateway");
    await page.getByLabel("Repository hint · optional").fill("github.com/acme/gateway");
    await page.getByRole("button", { name: "Create project" }).click();
    await expect(page.getByText("API gateway is created")).toBeVisible();
    await expect(page.getByText("Its address is http://localhost:4200/api-gateway.", { exact: false })).toBeVisible();
    const created = (await apiRequests()).find((r) => r.method === "POST" && r.path === "/api/v1/projects");
    expect(created?.body).toMatchObject({ name: "API gateway", slug: "api-gateway", remote_hint: "github.com/acme/gateway", tasks: [] });
    expect(typeof created?.body?.idempotency_key).toBe("string");
    await page.getByRole("link", { name: "Invite people" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/projects/api-gateway/invites`);
  });
});

test.describe("project settings", () => {
  test("renames the project and clears the hint; the address stays", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/kontur/settings");
    await expect(page.getByText("http://localhost:4200/kontur").last()).toBeVisible();
    const name = page.getByRole("textbox", { name: "Name" });
    await name.fill("Kontur core");
    await page.getByRole("button", { name: "Save" }).first().click();
    await expect(page.getByRole("heading", { name: "Kontur core" })).toBeVisible();
    await page.getByRole("textbox", { name: "Repository hint" }).fill("");
    await page.getByRole("button", { name: "Save" }).last().click();
    await expect(page.getByText("github.com/acme/kontur")).toHaveCount(0);
    expect((await apiRequests()).filter((r) => r.method === "PATCH").map((r) => r.body)).toEqual([{ name: "Kontur core" }, { remote_hint: "" }]);
  });

  test("an address that names no project says so", async ({ page }) => {
    await signIn(page);
    await page.goto("/admin/projects/nothing-here");
    await expect(page.getByText("No such project here")).toBeVisible();
  });
});
