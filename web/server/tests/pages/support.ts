import { expect, type Page } from "@playwright/test";

export const API = "http://127.0.0.1:4210";
export const CLOUD = "http://127.0.0.1:4220";
export const ADMIN = "http://localhost:4200";

export interface Recorded {
  method: string;
  path: string;
  forwardedFor: string | null;
  forwardedHost: string | null;
  forwardedProto: string | null;
  authorization: string | null;
  body: Record<string, unknown> | null;
}

/** Puts the fake team server in one of its scenarios (`tests/fake-api/fixtures.ts`), signed out. */
export async function reset(scenario = "full"): Promise<void> {
  await fetch(`${API}/__fake/reset`, { method: "POST", body: JSON.stringify({ scenario }) });
}

/** What the fake team server received since the last reset. */
export async function apiRequests(): Promise<Recorded[]> {
  const response = await fetch(`${API}/__fake/requests`);
  return (await response.json()) as Recorded[];
}

export async function deliveries(): Promise<{ how: string; email: string }[]> {
  const response = await fetch(`${CLOUD}/__fake/deliveries`);
  return (await response.json()) as { how: string; email: string }[];
}

/** Signs in with a code: `ADMN-0001` is Ivan, an admin; `OWNR-0001` is Maria, who owns Mobile app only. */
export async function signIn(page: Page, code = "ADMN-0001"): Promise<void> {
  await page.goto("/admin/signin/code");
  await page.getByLabel("Code").fill(code);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(`${ADMIN}/admin`);
}

/** Every request the browser made goes to the admin itself (or, on a trip, the cloud), never to the API. */
export function watchBrowserRequests(page: Page): string[] {
  const seen: string[] = [];
  page.on("request", (request) => seen.push(request.url()));
  return seen;
}

export function expectNoApiAddress(urls: string[]): void {
  expect(urls.filter((url) => url.startsWith(API))).toEqual([]);
}
