import { expect, test, type Page } from "@playwright/test";

import { API, apiRequests, EMAIL, PASSWORD } from "./support";

/** `/authorize`: a team server's admin asks the cloud to sign the person in (`sssAw`) or to link the server (`WNc6d`). */

const SERVER_ID = "0199f0a0-0000-7000-8000-00000000c10d";
const CALLBACK = `${API}/admin/auth/callback`;
const STATE = "state-from-the-admin-0123456789";

function statement(address: string): string {
  const part = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  const payload = { server: SERVER_ID, nonce: "n".repeat(32), name: "Kontur team", address, public_key: "key",
    fingerprint: "ab".repeat(32), issued_at: 1 };
  return `${part({ alg: "EdDSA", typ: "orchester-link+jws", kid: "k" })}.${part(payload)}.c2ln`;
}

function escape(text: string): string {
  return text.replace(/[&<>"']/g, (c) => `&#${String(c.charCodeAt(0))};`);
}

/**
 * What a team server's admin page does: a form posted to `/authorize/start`, the request in its body, from a page on
 * `from` (the admin's own origin, the callback's, unless a test says otherwise).
 */
async function start(page: Page, params: Record<string, string>, from = new URL(params.return ?? API).origin): Promise<void> {
  const fields = Object.entries(params).map(([name, value]) => `<input type="hidden" name="${name}" value="${escape(value)}">`);
  const html = `<form method="post" action="http://localhost:4100/authorize/start">${fields.join("")}<button>Go</button></form>`;
  await page.route(`${from}/__trip`, (route) => route.fulfill({ contentType: "text/html", body: html }));
  await page.goto(`${from}/__trip`);
  await page.getByRole("button", { name: "Go" }).click();
}

async function linked() {
  return (await apiRequests()).filter((request) => request.path === "/api/v1/servers" && request.method === "POST");
}

async function signIn(page: Page): Promise<void> {
  await page.getByLabel("Email").fill(EMAIL);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
}

async function callbacks() {
  return (await apiRequests()).filter((request) => request.path === "/admin/auth/callback");
}

test("a sign-in to a server's admin goes through sign-in, then posts the token to the admin's callback", async ({ page }) => {
  const before = (await callbacks()).length;
  await start(page, { purpose: "admin", server: SERVER_ID, state: STATE, return: CALLBACK });
  await expect(page).toHaveURL(/\/signin\?next=/);
  await signIn(page);
  await expect(page.getByRole("heading", { name: `Sign in to Kontur team's admin as ${EMAIL}?` })).toBeVisible();
  await expect(page.getByText("Back to 127.0.0.1:4110/admin")).toBeVisible();
  const csp = (await page.request.get(page.url())).headers()["content-security-policy"];
  expect(csp).toContain(`form-action 'self' ${API}`);
  await page.getByRole("button", { name: "Sign in to the admin" }).click();
  await expect(page).toHaveURL(CALLBACK, { timeout: 15_000 });
  const posted = (await callbacks()).slice(before);
  expect(posted).toHaveLength(1);
  expect(posted[0]?.body).toEqual({ state: STATE, identity_token: "fake-admin-token", key_set: "fake-key-set" });
});

test("linking shows the check code, posts the keys back, and the statement is in no address", async ({ page }) => {
  const addresses: string[] = [];
  page.on("request", (request) => addresses.push(request.url()));
  await page.goto("/signin");
  await signIn(page);
  await expect(page).toHaveURL(/\/account$/);
  const before = (await callbacks()).length;
  const signed = statement(API);
  await start(page, { purpose: "link", statement: signed, state: STATE, return: CALLBACK });
  await expect(page.getByRole("heading", { name: "Link Kontur team to your account?" })).toBeVisible();
  await expect(page.getByText("NENT-QAXB-NENT-QAXB")).toBeVisible();
  await page.getByRole("button", { name: "Link Kontur team" }).click();
  await expect(page).toHaveURL(CALLBACK);
  const posted = (await callbacks()).slice(before);
  expect(posted[0]?.body).toEqual({ state: STATE, identity_token: "fake-link-token", key_set: "fake-key-set",
    root_key: "fake-root-key" });
  const payload = signed.split(".")[1] ?? "-";
  expect(addresses.filter((url) => url.includes(payload))).toEqual([]);
});

test("a request another site swaps in after the page loaded is not confirmed", async ({ page, context }) => {
  // Security review 2, 1: confirming acts on the request the page showed, not on whatever the cookie holds by then.
  await page.goto("/signin");
  await signIn(page);
  await expect(page).toHaveURL(/\/account$/);
  await start(page, { purpose: "link", statement: statement(API), state: STATE, return: CALLBACK });
  await expect(page.getByRole("heading", { name: "Link Kontur team to your account?" })).toBeVisible();
  // Another admin's page (on loopback here, as the test cloud is plain HTTP) posts its own statement meanwhile.
  const attacker = await context.newPage();
  const evil = "http://localhost:4111";
  await start(attacker, { purpose: "link", statement: statement(evil), state: STATE, return: `${evil}/admin/auth/callback` });
  await expect(attacker.getByRole("heading", { name: "Link Kontur team to your account?" })).toBeVisible();
  await attacker.close();
  const before = (await linked()).length;
  await page.getByRole("button", { name: "Link Kontur team" }).click();
  await expect(page.getByText("This page is out of date, open the link again.")).toBeVisible();
  expect((await linked()).length).toBe(before);
});

test("a request posted from anywhere but the admin it answers is not kept", async ({ page }) => {
  await page.goto("/signin");
  await signIn(page);
  await expect(page).toHaveURL(/\/account$/);
  await start(page, { purpose: "link", statement: statement(API), state: STATE, return: CALLBACK });
  await expect(page.getByRole("heading", { name: "Link Kontur team to your account?" })).toBeVisible();
  const form = { purpose: "link", statement: statement(API), state: "another-state-0123456789abcdef", return: CALLBACK };
  const posts: Record<string, string>[] = [{}, { origin: "https://evil.example" }, { origin: "null" }];
  for (const headers of posts) {
    const answer = await page.request.post("/authorize/start", { form, headers, maxRedirects: 0 });
    expect(answer.status()).toBe(303);
    expect(answer.headers().location).toBe("http://localhost:4100/authorize");
  }
  // An https admin's page posting to this plain-HTTP test cloud sends `Origin: null`: taken only here, on loopback.
  const https = { ...form, return: "https://kontur.test/admin/auth/callback" };
  const downgraded = await page.request.post("/authorize/start", { form: https, headers: { origin: "null" }, maxRedirects: 0 });
  expect(downgraded.headers().location).toContain("/authorize?return=");
  await start(page, { purpose: "link", statement: statement(API), state: STATE, return: CALLBACK });
  // This browser's own request is still the one the page shows and confirms.
  await page.reload();
  const before = (await callbacks()).length;
  await page.getByRole("button", { name: "Link Kontur team" }).click();
  await expect(page).toHaveURL(CALLBACK);
  expect((await callbacks()).slice(before)[0]?.body).toMatchObject({ state: STATE });
});

test("a request in the address alone is not answered", async ({ page }) => {
  await page.goto("/signin");
  await signIn(page);
  await expect(page).toHaveURL(/\/account$/);
  const query = new URLSearchParams({ purpose: "link", statement: statement(API), state: STATE, return: CALLBACK });
  await page.goto(`/authorize?${query.toString()}`);
  await expect(page.getByRole("heading", { name: "This request can't be answered" })).toBeVisible();
});

test("an answer goes back only to the server the request is about", async ({ page }) => {
  await start(page, { purpose: "link", statement: statement("https://evil.example"), state: STATE, return: CALLBACK });
  if (page.url().includes("/signin")) {
    await signIn(page);
  }
  await expect(page.getByRole("heading", { name: "This request can't be answered" })).toBeVisible();
  await start(page, { purpose: "admin", server: SERVER_ID, state: STATE, return: "https://evil.example/elsewhere" });
  await expect(page.getByRole("heading", { name: "This request can't be answered" })).toBeVisible();
  // The page's own route state names the query; what matters is that the form carries no `next` to go to.
  const signInPage = await page.request.get("/signin?next=https://evil.example/");
  expect(await signInPage.text()).not.toContain('name="next"');
});

test("an invite carried by the inviter's browser is delivered, and the page leads back to its server's admin", async ({ page }) => {
  await page.setContent(`<form method="post" action="http://localhost:4100/invites/deliver">
    <input name="invite" value="a.b.c"><input name="return" value="https://evil.example/admin">
    <button>Send</button></form>`);
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByRole("heading", { name: "Invite delivered" })).toBeVisible();
  await expect(page).toHaveURL("http://localhost:4100/invites/delivered");
  // Security review 1, minor 2: back to the server the cloud registered, never to an address the post named.
  await expect(page.getByRole("link", { name: "Back to 127.0.0.1:4110" })).toHaveAttribute("href", `${API}/admin`);
  await page.goto("/invites/delivered?outcome=sent&return=https://evil.example/admin");
  await expect(page.locator("a[href*='evil.example']")).toHaveCount(0);
  const delivered = (await apiRequests()).filter((request) => request.path === "/api/v1/invites" && request.method === "POST");
  expect(delivered.at(-1)?.body).toEqual({ invite: "a.b.c" });
});
