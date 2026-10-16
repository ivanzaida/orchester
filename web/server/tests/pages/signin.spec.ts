import { expect, test } from "@playwright/test";

import { ADMIN, authorizePosts, apiRequests, expectNoApiAddress, reset, signIn, watchBrowserRequests } from "./support";

test.beforeEach(async () => {
  await reset();
});

test.describe("sign in with a code", () => {
  test("signs in without the browser holding the token or calling the API, then signs out", async ({ page, context }) => {
    const urls = watchBrowserRequests(page);
    await signIn(page);
    await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
    await expect(page.getByRole("banner").getByText("Admin · signed in with a code").first()).toBeVisible();

    const signInCall = (await apiRequests()).find((r) => r.path === "/api/v1/auth/code");
    expect(signInCall?.body).toMatchObject({ kind: "admin_web", device: "Edge on Windows", code: "<redacted>" });
    const token = (await apiRequests()).filter((r) => r.authorization !== null).at(-1)?.authorization?.replace("Bearer ", "") ?? "";
    expect(token).toMatch(/^adm_fake_/);

    const cookies = await context.cookies();
    const session = cookies.find((cookie) => cookie.name === "__Host-orchester_admin");
    expect(session).toMatchObject({ httpOnly: true, secure: true, sameSite: "Lax", path: "/" });
    for (const cookie of cookies) {
      expect(cookie.value).not.toContain(token);
    }
    expect(await page.content()).not.toContain(token);
    expect(await page.evaluate(() => document.cookie)).toBe("");
    expectNoApiAddress(urls);

    await page.getByRole("banner").getByRole("button", { name: "Sign out" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/signin`);
    expect((await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_admin")).toBe(false);
    expect((await apiRequests()).some((r) => r.method === "DELETE" && r.path === "/api/v1/auth/sessions/current")).toBe(true);
  });

  test("a wrong, used or expired code is one answer, and the code stays to correct", async ({ page }) => {
    await page.goto("/admin/signin/code");
    await page.getByLabel("Code").fill("H8RW-2NCF");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText("That code isn't right, or it was used or has expired.", { exact: false })).toBeVisible();
    await expect(page.getByLabel("Code")).toHaveValue("H8RW-2NCF");
    await expect(page.getByLabel("Code")).toHaveAttribute("aria-invalid", "true");
  });

  test("too many wrong codes stop the button until the time the server gave", async ({ page }) => {
    await page.goto("/admin/signin/code");
    await page.getByLabel("Code").fill("LIMT-0001");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText("Too many wrong codes from this address")).toBeVisible();
    await expect(page.getByText(/Try again at \d\d:\d\d\./)).toBeVisible();
    await expect(page.getByRole("button", { name: "Sign in" })).toBeDisabled();
  });

  test("someone with nothing to manage is told so, and nothing is kept", async ({ page, context }) => {
    await page.goto("/admin/signin/code");
    await page.getByLabel("Code").fill("NONE-0001");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/nothing`);
    await expect(page.getByRole("heading", { name: "Nothing to manage here" })).toBeVisible();
    expect((await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_admin")).toBe(false);
    await page.getByRole("button", { name: "Sign out" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/signin`);
  });

  test("the pages need a session", async ({ page }) => {
    for (const path of ["/admin", "/admin/people", "/admin/projects/kontur", "/admin/settings"]) {
      await page.goto(path);
      await expect(page).toHaveURL(`${ADMIN}/admin/signin`);
    }
  });
});

test.describe("sign in with Orchester Cloud", () => {
  test("goes to the cloud's /authorize with a state and comes back signed in", async ({ page, context }) => {
    await page.goto("/admin/signin");
    await page.getByRole("button", { name: "Sign in with Orchester Cloud" }).click();
    await expect(page).toHaveURL("http://127.0.0.1:4220/authorize/start");
    const trip = (await authorizePosts()).at(-1);
    expect(trip).toMatchObject({ purpose: "admin", return: `${ADMIN}/admin/auth/callback` });
    const state = (await context.cookies()).find((cookie) => cookie.name === "__Host-orchester_admin_state");
    expect(state).toMatchObject({ httpOnly: true, secure: true, sameSite: "None", path: "/" });
    expect(state?.value).not.toContain(trip?.state ?? "-");

    await page.getByRole("button", { name: "Continue as ivan@example.com" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin`);
    await expect(page.getByRole("banner").getByText("Admin · signed in with Orchester Cloud").first()).toBeVisible();
    expect((await context.cookies()).some((cookie) => cookie.name === "__Host-orchester_admin_state")).toBe(false);
    const call = (await apiRequests()).find((r) => r.path === "/api/v1/auth/cloud");
    expect(call?.body).toMatchObject({ kind: "admin_web", key_set: "fake-key-set", identity_token: "<redacted>" });
  });

  test("an account the server does not know is told so on the sign-in page", async ({ page }) => {
    await page.goto("/admin/signin");
    await page.getByRole("button", { name: "Sign in with Orchester Cloud" }).click();
    await page.getByRole("button", { name: "Continue as stranger@example.com" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/signin?failed=not_invited`);
    await expect(page.getByText("There's no account on Kontur team for that Orchester account.", { exact: false })).toBeVisible();
  });

  test("nothing to manage names who the cloud vouched for", async ({ page }) => {
    await page.goto("/admin/signin");
    await page.getByRole("button", { name: "Sign in with Orchester Cloud" }).click();
    await page.getByRole("button", { name: "Continue as dana@example.com" }).click();
    await expect(page).toHaveURL(`${ADMIN}/admin/nothing`);
    await expect(page.getByText("You're signed in as Dana Ruiz (dana@example.com).", { exact: false })).toBeVisible();
  });

  test("a callback whose state is not this browser's is refused", async ({ page, request }) => {
    const answer = await request.post("/admin/auth/callback", { form: { state: "forged", identity_token: "x.y.z" }, maxRedirects: 0 });
    expect(answer.status()).toBe(303);
    expect(answer.headers().location).toContain("/admin/signin?failed=trip");
    expect((await apiRequests()).some((r) => r.path === "/api/v1/auth/cloud")).toBe(false);
    await page.goto("/admin/signin?failed=trip");
    await expect(page.getByText("That sign-in took too long or came back to another browser.", { exact: false })).toBeVisible();
  });

  test("cloud sign-ins turned off are said, and the code is offered", async ({ page }) => {
    await reset("cloud-off");
    await page.goto("/admin/signin");
    await expect(page.getByText("Sign-ins through Orchester Cloud are off")).toBeVisible();
    await expect(page.getByRole("button", { name: "Sign in with Orchester Cloud" })).toHaveCount(0);
    await expect(page.getByRole("link", { name: "Sign in with a code" })).toBeVisible();
  });

  test("the first-visit certificate check shows the fingerprint and check code", async ({ page }) => {
    await page.goto("/admin/signin");
    await expect(page.getByText("First visit: is this your server?")).toBeVisible();
    await expect(page.getByText("4F:7A:92:C1:0D:8E:51:B3 … 6A:2F:C0:91")).toBeVisible();
    await expect(page.getByText("3F7A-92C1-KD8M-QX4T")).toBeVisible();
    await reset("plain");
    await page.reload();
    await expect(page.getByText("First visit: is this your server?")).toHaveCount(0);
  });
});
