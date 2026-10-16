import type { TripBack, TripPurpose } from "@/lib/server/session";

/**
 * A trip to Orchester Cloud's `/authorize` (contract, "The trip to the cloud's /authorize"). The cloud sends the
 * browser back with a form `POST` to `return`, which must be this admin's callback.
 */
export const CALLBACK_PATH = "/admin/auth/callback";

export type AuthorizeTarget =
  | { readonly purpose: "admin"; readonly server: string }
  | { readonly purpose: "link"; readonly statement: string };

/**
 * Where a trip's form goes: the cloud's `/authorize/start` takes it as a `POST` and keeps it server-side, so the link
 * statement never travels in a URL (security review 1, B1).
 */
export function authorizeStart(cloudOrigin: string): string {
  return new URL("/authorize/start", cloudOrigin).toString();
}

/** The fields of the trip's form to the cloud. */
export function authorizeFields(ownOrigin: string, state: string, target: AuthorizeTarget): [string, string][] {
  const own: [string, string] = target.purpose === "admin" ? ["server", target.server] : ["statement", target.statement];
  return [["purpose", target.purpose], own, ["state", state], ["return", `${ownOrigin}${CALLBACK_PATH}`]];
}

/** Where a trip's outcome is shown: the sign-in page, or the page that started a link. */
export function backPath(purpose: TripPurpose, back: TripBack): string {
  if (purpose === "admin") {
    return "/admin/signin";
  }
  return back === "setup" ? "/admin/setup/cloud" : "/admin/settings";
}

/** A page address with one outcome word in its query: only fixed words, never a token or a code. */
export function withOutcome(path: string, name: "failed" | "link", outcome: string): string {
  return `${path}?${new URLSearchParams({ [name]: outcome }).toString()}`;
}
