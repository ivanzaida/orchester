import type { TripBack, TripPurpose } from "@/lib/server/session";

/**
 * The address of Orchester Cloud's `/authorize` for a trip (contract, "The trip to the cloud's /authorize"). The cloud
 * sends the browser back with a form `POST` to `return`, which must be this admin's callback.
 */
export const CALLBACK_PATH = "/admin/auth/callback";

export type AuthorizeTarget =
  | { readonly purpose: "admin"; readonly server: string }
  | { readonly purpose: "link"; readonly statement: string };

export function authorizeUrl(cloudOrigin: string, ownOrigin: string, state: string, target: AuthorizeTarget): string {
  const url = new URL("/authorize", cloudOrigin);
  url.searchParams.set("purpose", target.purpose);
  if (target.purpose === "admin") {
    url.searchParams.set("server", target.server);
  } else {
    url.searchParams.set("statement", target.statement);
  }
  url.searchParams.set("state", state);
  url.searchParams.set("return", `${ownOrigin}${CALLBACK_PATH}`);
  return url.toString();
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
