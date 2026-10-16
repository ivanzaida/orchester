import { test } from "@playwright/test";

import { API, EMAIL, PASSWORD } from "./support";

/**
 * Screenshots of `/authorize` for comparing with `sssAw` and `WNc6d`, at the frames' 1440 and a phone's 390. Only
 * when `SHOTS` names a folder: `SHOTS=<folder> npx playwright test shots`.
 */
const folder = process.env.SHOTS;
const SERVER_ID = "0199f0a0-0000-7000-8000-00000000c10d";
const STATE = "state-from-the-admin-0123456789";
const CALLBACK = `${API}/admin/auth/callback`;

function statement(): string {
  const part = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  const payload = { server: SERVER_ID, name: "Kontur team", address: API, public_key: "key", fingerprint: "ab".repeat(32),
    issued_at: 1 };
  return `${part({ alg: "EdDSA", typ: "orchester-link+jws" })}.${part(payload)}.c2ln`;
}

test.skip(folder === undefined, "screenshots only when SHOTS is set");
test.use({ colorScheme: "dark" });

test("authorize screenshots", async ({ page }) => {
  await page.goto("/signin");
  await page.getByLabel("Email").fill(EMAIL);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await page.waitForURL(/\/account$/);
  const admin = new URLSearchParams({ purpose: "admin", server: SERVER_ID, state: STATE, return: CALLBACK });
  const link = new URLSearchParams({ purpose: "link", statement: statement(), state: STATE, return: CALLBACK });
  for (const [name, query] of [["sssAw", admin], ["WNc6d", link]] as const) {
    for (const width of [1440, 390]) {
      await page.setViewportSize({ width, height: width === 1440 ? 900 : 844 });
      await page.goto(`/authorize?${query.toString()}`);
      await page.screenshot({ path: `${String(folder)}/cloud-${name}-${String(width)}.png`, fullPage: true });
    }
  }
});
