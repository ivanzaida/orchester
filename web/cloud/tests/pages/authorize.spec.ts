import { expect, test } from "@playwright/test";

import { API, apiRequests, EMAIL, PASSWORD } from "./support";

/** `/authorize`: a team server's admin asks the cloud to sign the person in (`sssAw`) or to link the server (`WNc6d`). */

const SERVER_ID = "0199f0a0-0000-7000-8000-00000000c10d";
const CALLBACK = `${API}/admin/auth/callback`;
const STATE = "state-from-the-admin-0123456789";

function query(params: Record<string, string>): string {
  return `/authorize?${new URLSearchParams(params).toString()}`;
}

function statement(address: string): string {
  const part = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  const payload = { server: SERVER_ID, name: "Kontur team", address, public_key: "key", fingerprint: "ab".repeat(32),
    issued_at: 1 };
  return `${part({ alg: "EdDSA", typ: "orchester-link+jws", kid: "k" })}.${part(payload)}.c2ln`;
}

async function callbacks() {
  return (await apiRequests()).filter((request) => request.path === "/admin/auth/callback");
}

test("a sign-in to a server's admin goes through sign-in, then posts the token to the admin's callback", async ({ page }) => {
  const before = (await callbacks()).length;
  await page.goto(query({ purpose: "admin", server: SERVER_ID, state: STATE, return: CALLBACK }));
  await expect(page).toHaveURL(/\/signin\?next=/);
  await page.getByLabel("Email").fill(EMAIL);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: `Sign in to Kontur team's admin as ${EMAIL}?` })).toBeVisible();
  await expect(page.getByText("Back to 127.0.0.1:4110/admin")).toBeVisible();
  const csp = (await page.request.get(query({ purpose: "admin", server: SERVER_ID, state: STATE, return: CALLBACK })))
    .headers()["content-security-policy"];
  expect(csp).toContain(`form-action 'self' ${API}`);
  await page.getByRole("button", { name: "Sign in to the admin" }).click();
  await expect(page).toHaveURL(CALLBACK, { timeout: 15_000 });
  const posted = (await callbacks()).slice(before);
  expect(posted).toHaveLength(1);
  expect(posted[0]?.body).toEqual({ state: STATE, identity_token: "fake-admin-token", key_set: "fake-key-set" });
});

test("linking shows the server's check code and posts its keys back", async ({ page }) => {
  await page.goto("/signin");
  await page.getByLabel("Email").fill(EMAIL);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/account$/);
  const before = (await callbacks()).length;
  await page.goto(query({ purpose: "link", statement: statement(API), state: STATE, return: CALLBACK }));
  await expect(page.getByRole("heading", { name: "Link Kontur team to your account?" })).toBeVisible();
  await expect(page.getByText("NENT-QAXB-NENT-QAXB")).toBeVisible();
  await page.getByRole("button", { name: "Link Kontur team" }).click();
  await expect(page).toHaveURL(CALLBACK);
  const posted = (await callbacks()).slice(before);
  expect(posted[0]?.body).toEqual({ state: STATE, identity_token: "fake-link-token", key_set: "fake-key-set",
    root_key: "fake-root-key" });
});

test("an answer goes back only to the server the request is about", async ({ page }) => {
  await page.goto(query({ purpose: "link", statement: statement("https://evil.example"), state: STATE, return: CALLBACK }));
  if (page.url().includes("/signin")) {
    await page.getByLabel("Email").fill(EMAIL);
    await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
    await page.getByRole("button", { name: "Sign in" }).click();
  }
  await expect(page.getByRole("heading", { name: "This request can't be answered" })).toBeVisible();
  await page.goto(query({ purpose: "admin", server: SERVER_ID, state: STATE, return: "https://evil.example/elsewhere" }));
  await expect(page.getByRole("heading", { name: "This request can't be answered" })).toBeVisible();
  // The page's own route state names the query; what matters is that the form carries no `next` to go to.
  const signIn = await page.request.get("/signin?next=https://evil.example/");
  expect(await signIn.text()).not.toContain('name="next"');
});

test("an invite carried by the inviter's browser is delivered, and the page leads back to the admin", async ({ page }) => {
  await page.setContent(`<form method="post" action="http://localhost:4100/invites/deliver">
    <input name="invite" value="a.b.c"><input name="return" value="${API}/admin/projects/kontur/invites">
    <button>Send</button></form>`);
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByRole("heading", { name: "Invite delivered" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Back to 127.0.0.1:4110" })).toHaveAttribute("href",
    `${API}/admin/projects/kontur/invites`);
  const delivered = (await apiRequests()).filter((request) => request.path === "/api/v1/invites" && request.method === "POST");
  expect(delivered.at(-1)?.body).toEqual({ invite: "a.b.c" });
});
