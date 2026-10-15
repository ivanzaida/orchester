import { headers } from "next/headers";
import { NextResponse } from "next/server";

import { field } from "@/lib/form";
import type { ApiFailure } from "@/lib/server/api";
import { backPath, withOutcome } from "@/lib/server/authorize";
import { browserDevice } from "@/lib/server/device";
import { namedIn } from "@/lib/server/identity";
import { tripMatches, type Trip } from "@/lib/server/session";
import { signInWithCloud } from "@/lib/server/team/auth";
import { changeCloud } from "@/lib/server/team/settings";
import { anonymous, endSession, endTrip, setNothingFlow, startSession, trip } from "@/lib/server/visitor";

/**
 * Where Orchester Cloud's `/authorize` sends the browser back, with a form `POST` from the cloud's origin (contract,
 * "The trip to the cloud's /authorize"). The posted `state` must match the trip's cookie: that is this route's CSRF
 * check, since the `Origin` is the cloud's. The cookie is deleted either way.
 */
function to(request: Request, path: string): NextResponse {
  return NextResponse.redirect(new URL(path, request.url), 303);
}

const SIGN_IN_REFUSALS = new Set(["not_invited", "cloud_off", "key_set_stale", "invalid_identity", "rate_limited"]);

function signInOutcome(failure: ApiFailure): string {
  return failure.kind === "refused" && SIGN_IN_REFUSALS.has(failure.code) ? failure.code : "unavailable";
}

async function finishSignIn(request: Request, form: FormData): Promise<NextResponse> {
  const identityToken = field(form, "identity_token");
  const keySet = field(form, "key_set");
  const device = browserDevice((await headers()).get("user-agent"));
  const result = await signInWithCloud({ identityToken, keySet: keySet === "" ? null : keySet, device }, await anonymous());
  if (result.ok) {
    await startSession({ token: result.value.token, via: "cloud" });
    return to(request, "/admin");
  }
  if (result.error.kind === "refused" && result.error.code === "nothing_to_manage") {
    await setNothingFlow(namedIn(identityToken));
    return to(request, "/admin/nothing");
  }
  return to(request, withOutcome("/admin/signin", "failed", signInOutcome(result.error)));
}

function linkOutcome(failure: ApiFailure): string {
  if (failure.kind !== "refused") {
    return "unavailable";
  }
  return failure.code === "conflict" || failure.code === "invalid" || failure.code === "forbidden" ? failure.code : "refused";
}

async function finishLink(request: Request, form: FormData, started: Trip): Promise<NextResponse> {
  const page = backPath("link", started.back);
  if (started.session === null) {
    return to(request, "/admin/signin");
  }
  const link = { root_key: field(form, "root_key"), key_set: field(form, "key_set"), identity_token: field(form, "identity_token") };
  const result = await changeCloud({ link }, { ...(await anonymous()), token: started.session.token });
  if (!result.ok && result.error.kind === "refused" && result.error.code === "unauthorized") {
    await endSession();
    return to(request, "/admin/signin");
  }
  if (!result.ok) {
    return to(request, withOutcome(page, "link", linkOutcome(result.error)));
  }
  return to(request, started.back === "setup" ? "/admin/setup/done" : withOutcome(page, "link", "done"));
}

export async function POST(request: Request): Promise<NextResponse> {
  const form = await request.formData();
  const started = await trip();
  await endTrip();
  if (started === null) {
    return to(request, withOutcome("/admin/signin", "failed", "trip"));
  }
  if (!tripMatches(started, form.get("state"))) {
    return to(request, withOutcome(backPath(started.purpose, started.back), started.purpose === "link" ? "link" : "failed", "trip"));
  }
  return started.purpose === "admin" ? finishSignIn(request, form) : finishLink(request, form, started);
}

/** The callback is only ever posted to; a bare visit starts over at sign-in. */
export function GET(request: Request): NextResponse {
  return to(request, "/admin/signin");
}
