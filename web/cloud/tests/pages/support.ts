import { expect, type Page } from "@playwright/test";

export const API = "http://127.0.0.1:4110";
export const EMAIL = "mira.koval@example.com";
export const PASSWORD = "correct horse battery";

export interface Recorded {
  method: string;
  path: string;
  forwardedFor: string | null;
  authorization: string | null;
  cookie: string | null;
  body: Record<string, unknown> | null;
}

/** What the fake API received so far. */
export async function apiRequests(): Promise<Recorded[]> {
  const response = await fetch(`${API}/__fake/requests`);
  return (await response.json()) as Recorded[];
}

/** Puts the fake's accounts back as drawn, for a test that changes them. */
export async function resetFake(): Promise<void> {
  await fetch(`${API}/__fake/reset`, { method: "POST" });
}

/** Every request the browser made goes to the site itself, never to the API. */
export function watchBrowserRequests(page: Page): string[] {
  const seen: string[] = [];
  page.on("request", (request) => seen.push(request.url()));
  return seen;
}

export async function signInAs(page: Page, email = EMAIL, password = PASSWORD): Promise<void> {
  await page.goto("/signin");
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
}

export function expectNoApiAddress(urls: string[]): void {
  expect(urls.filter((url) => url.startsWith(API))).toEqual([]);
  expect(urls.every((url) => url.startsWith("http://localhost:4100/"))).toBe(true);
}

/** Ends the browser's session at the cloud behind its back, as a password reset or a sign-out elsewhere does. */
export async function revokeAtCloud(): Promise<void> {
  const bearer = (await apiRequests()).filter((r) => r.path === "/api/v1/me" && r.authorization !== null).at(-1)?.authorization;
  if (bearer === undefined || bearer === null) {
    throw new Error("no session seen at the fake API");
  }
  await fetch(`${API}/api/v1/auth/sessions/current`, { method: "DELETE", headers: { authorization: bearer } });
}
