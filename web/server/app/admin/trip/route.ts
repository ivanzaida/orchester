import { NextResponse } from "next/server";

import { field } from "@/lib/form";
import { authorizeUrl, backPath, withOutcome } from "@/lib/server/authorize";
import { config } from "@/lib/server/config";
import { sessionEnded } from "@/lib/server/failures";
import { newState, type TripBack } from "@/lib/server/session";
import { serverInfo } from "@/lib/server/team/auth";
import { changeCloud } from "@/lib/server/team/settings";
import { anonymous, endSession, formIsGenuine, ownOrigin, session, startTrip } from "@/lib/server/visitor";

/**
 * Starts a trip to Orchester Cloud's `/authorize` (contract, "The trip to the cloud's /authorize"): *Sign in with
 * Orchester Cloud* (`purpose=admin`) or *Link to Orchester Cloud* (`purpose=link`). A plain form `POST` with the CSRF
 * token; the answer sends the browser to the cloud (303) with a fresh `state` kept in a cookie.
 */
function to(request: Request, path: string): NextResponse {
  return NextResponse.redirect(new URL(path, request.url), 303);
}

async function signInTrip(request: Request, cloud: string): Promise<NextResponse> {
  const info = await serverInfo(await anonymous());
  if (!info.ok) {
    return to(request, withOutcome("/admin/signin", "failed", "unavailable"));
  }
  if (info.value.cloud === null || !info.value.cloud.accepting) {
    return to(request, withOutcome("/admin/signin", "failed", "cloud_off"));
  }
  const state = newState();
  await startTrip({ state, purpose: "admin", back: "signin", session: null });
  const target = authorizeUrl(cloud, await ownOrigin(), state, { purpose: "admin", server: info.value.cloud.server });
  return NextResponse.redirect(target, 303);
}

async function linkTrip(request: Request, cloud: string, back: TripBack): Promise<NextResponse> {
  const current = await session();
  if (current === null) {
    return to(request, "/admin/signin");
  }
  const result = await changeCloud({ link: "start" }, { ...(await anonymous()), token: current.token });
  if (!result.ok && sessionEnded(result.error)) {
    await endSession();
    return to(request, "/admin/signin");
  }
  if (!result.ok || result.value.statement === null) {
    return to(request, withOutcome(backPath("link", back), "link", "unavailable"));
  }
  const state = newState();
  await startTrip({ state, purpose: "link", back, session: current });
  const target = authorizeUrl(cloud, await ownOrigin(), state, { purpose: "link", statement: result.value.statement });
  return NextResponse.redirect(target, 303);
}

export async function POST(request: Request): Promise<NextResponse> {
  const form = await request.formData();
  const purpose = field(form, "purpose");
  const back: TripBack = field(form, "back") === "setup" ? "setup" : "settings";
  const page = purpose === "link" ? backPath("link", back) : "/admin/signin";
  if (!(await formIsGenuine(form))) {
    return to(request, withOutcome(page, purpose === "link" ? "link" : "failed", "expired"));
  }
  const cloud = config().cloudOrigin;
  if (cloud === null) {
    return to(request, page);
  }
  return purpose === "link" ? linkTrip(request, cloud, back) : signInTrip(request, cloud);
}
