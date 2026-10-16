import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { CLOUD, reset, signIn } from "./support";

test.beforeEach(async () => {
  await reset();
});

test.describe("headers", () => {
  test("every page has a strict CSP with a fresh nonce; forms may go only here and to the cloud", async ({ request }) => {
    const first = await request.get("/admin/signin");
    const second = await request.get("/admin/signin");
    const policy = first.headers()["content-security-policy"] ?? "";
    expect(policy).toContain("default-src 'self'");
    expect(policy).toMatch(/script-src 'self' 'nonce-[\w-]+' 'strict-dynamic'(;|$)/);
    expect(policy).not.toContain("unsafe-inline");
    expect(policy).not.toContain("unsafe-eval");
    expect(policy).toContain(`form-action 'self' ${CLOUD}`);
    expect(policy).toContain("frame-ancestors 'none'");
    expect(policy).not.toBe(second.headers()["content-security-policy"]);
    expect(first.headers()["referrer-policy"]).toBe("strict-origin");
    expect(first.headers()["x-frame-options"]).toBe("DENY");
  });

  test("plain HTTP away from the server itself shows only *This page needs HTTPS*", async ({ request }) => {
    const headers = { "x-forwarded-host": "10.0.4.20:8740", "x-forwarded-proto": "http" };
    for (const path of ["/admin", "/admin/signin", "/admin/projects/kontur"]) {
      const answer = await request.get(path, { headers, maxRedirects: 0 });
      expect(answer.status(), path).toBe(200);
      const text = await answer.text();
      expect(text).toContain("This page needs HTTPS");
      expect(text).toContain("https://10.0.4.20:8740/admin");
      // Design 1, m20: the server itself as drawn, on the port the browser used.
      expect(text).toContain("on the server itself, http://127.0.0.1:8740/admin.");
    }
    const post = await request.post("/admin/trip", { headers, form: { purpose: "admin" }, maxRedirects: 0 });
    expect(post.headers().location ?? "").not.toContain(CLOUD);
  });

  test("the HTTPS page names only the address the browser used, never one a link or a header put on it", async ({ page }) => {
    // Security review 1, minor 2: `?origin=` and a client's own header of the proxy's are ignored.
    await page.setExtraHTTPHeaders({ "x-forwarded-host": "10.0.4.20:8740", "x-forwarded-proto": "http",
      "x-orchester-https-origin": "http://evil.example" });
    for (const path of ["/admin/https?origin=http://evil.example", "/admin?origin=http://evil.example"]) {
      await page.goto(path);
      const text = await page.locator("body").innerText();
      expect(text, path).toContain("https://10.0.4.20:8740/admin");
      expect(text, path).not.toContain("evil.example");
    }
  });

  test("the page loads nothing from another site and runs under its CSP", async ({ page }) => {
    const violations: string[] = [];
    page.on("console", (message) => {
      if (message.text().includes("Content Security Policy")) {
        violations.push(message.text());
      }
    });
    const urls: string[] = [];
    page.on("request", (request) => urls.push(request.url()));
    await signIn(page);
    await page.goto("/admin/projects/kontur/invites");
    await page.getByRole("button", { name: "Make a code" }).click();
    await expect(page.getByText("K7QM-3XRD")).toBeVisible();
    expect(urls.every((url) => url.startsWith("http://localhost:4200/"))).toBe(true);
    expect(violations).toEqual([]);
  });
});

test.describe("forms", () => {
  test("a form whose token does not match is refused", async ({ page }) => {
    await page.goto("/admin/signin/code");
    await page.getByLabel("Code").fill("ADMN-0001");
    await page.locator('input[name="csrf"]').evaluate((input: HTMLInputElement) => {
      input.value = "forged";
    });
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText("This form has expired. Reload the page and try again.")).toBeVisible();
  });

  test("a form posted from another site is refused", async ({ request }) => {
    const answer = await request.post("/admin/trip", { headers: { origin: "https://evil.example" }, form: { purpose: "admin", csrf: "x" }, maxRedirects: 0 });
    expect(answer.status()).toBe(303);
    expect(answer.headers().location).toContain("/admin/signin?failed=expired");
  });

  test("work without script, checked on the server", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto("/admin/signin/code");
    await page.getByLabel("Code").fill("H8RW-2NCF");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText("That code isn't right, or it was used or has expired.", { exact: false })).toBeVisible();
    await context.close();
  });
});

test.describe("a project's own address", () => {
  test("says to open it in Orchester and names nothing", async ({ page }) => {
    await page.goto("/kontur");
    await expect(page.getByRole("heading", { name: "Open this in Orchester" })).toBeVisible();
    await expect(page.getByText("http://localhost:4200/kontur is a project's address.", { exact: false })).toBeVisible();
    await page.goto("/no-such-project");
    await expect(page.getByRole("heading", { name: "Open this in Orchester" })).toBeVisible();
  });
});

test.describe("browser bundles", () => {
  test("hold no API address, token header or secret name", () => {
    const dir = join(process.cwd(), ".next", "static", "chunks");
    const files = readdirSync(dir, { recursive: true, withFileTypes: true }).filter((entry) => entry.name.endsWith(".js"));
    expect(files.length).toBeGreaterThan(0);
    for (const entry of files) {
      const text = readFileSync(join(entry.parentPath, entry.name), "utf8");
      for (const leak of ["ORCHESTER_ADMIN_SERVER_URL", "ORCHESTER_ADMIN_SECRET", "/api/v1", "Bearer "]) {
        expect(text.includes(leak), `${entry.name} contains ${leak}`).toBe(false);
      }
    }
  });
});
