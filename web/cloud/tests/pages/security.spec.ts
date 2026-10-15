import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test } from "@playwright/test";

import { EMAIL } from "./support";

test.describe("headers", () => {
  test("every page has a strict CSP with a fresh nonce, a referrer without path, and no framing", async ({ request }) => {
    const first = await request.get("/signin");
    const second = await request.get("/signin");
    const policy = first.headers()["content-security-policy"] ?? "";
    expect(policy).toContain("default-src 'self'");
    expect(policy).toMatch(/script-src 'self' 'nonce-[\w-]+' 'strict-dynamic'(;|$)/);
    expect(policy).not.toContain("unsafe-inline");
    expect(policy).not.toContain("unsafe-eval");
    expect(policy).toContain("frame-ancestors 'none'");
    expect(policy).not.toBe(second.headers()["content-security-policy"]);
    expect(first.headers()["referrer-policy"]).toBe("strict-origin");
    expect(first.headers()["x-frame-options"]).toBe("DENY");
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
    await page.goto("/signup");
    await page.getByLabel("Password", { exact: true }).fill("x");
    await page.getByRole("button", { name: "Show password" }).click();
    await expect(page.getByLabel("Password", { exact: true })).toHaveAttribute("type", "text");
    expect(urls.every((url) => url.startsWith("http://localhost:4100/"))).toBe(true);
    expect(violations).toEqual([]);
  });
});

test.describe("forms", () => {
  test("a form whose token does not match is refused", async ({ page }) => {
    await page.goto("/forgot");
    await page.getByLabel("Email").fill(EMAIL);
    await page.locator('input[name="csrf"]').evaluate((input: HTMLInputElement) => {
      input.value = "forged";
    });
    await page.getByRole("button", { name: "Send reset link" }).click();
    await expect(page.getByText("This form has expired. Reload the page and try again.")).toBeVisible();
    await expect(page).toHaveURL(/\/forgot$/);
  });

  test("work without script, checked on the server", async ({ browser }) => {
    const context = await browser.newContext({ javaScriptEnabled: false });
    const page = await context.newPage();
    await page.goto("/signin");
    await page.getByLabel("Email").fill(EMAIL);
    await page.getByLabel("Password", { exact: true }).fill("not the right one");
    await page.getByRole("button", { name: "Sign in" }).click();
    await expect(page.getByText("The email or password is not right. Check both, or reset your password.")).toBeVisible();
    await expect(page.getByLabel("Email")).toHaveValue(EMAIL);
    await expect(page.getByRole("button", { name: "Show password" })).toHaveCount(0);
    await context.close();
  });
});

test.describe("browser bundles", () => {
  test("hold no API address, token header or secret name", () => {
    const dir = join(process.cwd(), ".next", "static", "chunks");
    const files = readdirSync(dir, { recursive: true, withFileTypes: true }).filter((entry) => entry.name.endsWith(".js"));
    expect(files.length).toBeGreaterThan(0);
    for (const entry of files) {
      const text = readFileSync(join(entry.parentPath, entry.name), "utf8");
      for (const leak of ["ORCHESTER_CLOUD_API_URL", "ORCHESTER_CLOUD_WEB_SECRET", "/api/v1", "Bearer "]) {
        expect(text.includes(leak), `${entry.name} contains ${leak}`).toBe(false);
      }
    }
  });
});
