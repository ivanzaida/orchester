"use server";

import { readStatement, returnsToServer, type AuthorizeRequest } from "@/lib/authorize";
import { message, type Message } from "@/lib/i18n";
import type { ApiFailure, ApiResult } from "@/lib/server/api";
import { failureMessage } from "@/lib/server/failures";
import { adminToken, keySet, linkedServer, linkServer, type Answer } from "@/lib/server/registry";
import { anonymous, authorizeRequest, formIsGenuine, leaveIfEnded, sameAuthorizeRequest, signedIn } from "@/lib/server/visitor";
import { field } from "@/lib/form";

/**
 * Confirming on `/authorize`: the cloud issues what the server admin asked for, and the page carries it there with a
 * form post to the admin's callback. The token goes only into that post, never into a URL or a log.
 */
export type AuthorizeState =
  | { readonly kind: "ready" }
  | { readonly kind: "failed"; readonly error: Message }
  | { readonly kind: "go"; readonly action: string; readonly fields: readonly (readonly [string, string])[] };

function failed(key: Parameters<typeof message>[0]): AuthorizeState {
  return { kind: "failed", error: message(key) };
}

function refusal(failure: ApiFailure): AuthorizeState {
  if (failure.kind === "refused" && failure.code === "conflict") {
    return failed("authorize.linkedElsewhere");
  }
  if (failure.kind === "refused" && failure.code === "not_found") {
    return failed("authorize.notLinked");
  }
  if (failure.kind === "refused" && failure.code === "invalid") {
    return failed("authorize.statementRefused");
  }
  if (failure.kind === "refused" && failure.code === "keys_unavailable") {
    return failed("authorize.keysUnavailable");
  }
  return { kind: "failed", error: failureMessage(failure, "authorize.tooMany", Date.now()) };
}

async function answer(request: AuthorizeRequest, session: { client?: string | undefined; token: string }): Promise<ApiResult<Answer>> {
  if (request.purpose === "link") {
    return linkServer(request.statement, session);
  }
  const server = await linkedServer(request.server, session);
  if (!server.ok) {
    return server;
  }
  if (!returnsToServer(request.returnTo, server.value.address)) {
    return { ok: false, error: { kind: "refused", status: 400, code: "bad_request" } };
  }
  const token = await adminToken(request.server, session);
  if (!token.ok) {
    return token;
  }
  const keys = await keySet(await anonymous());
  return keys.ok ? { ok: true, value: { identityToken: token.value, keySet: keys.value } } : keys;
}

export async function authorizeAction(_previous: AuthorizeState, form: FormData): Promise<AuthorizeState> {
  if (!(await formIsGenuine(form))) {
    return failed("error.formExpired");
  }
  // The request this browser brought, from its sealed cookie: never from the form, never from a URL.
  const request = await authorizeRequest();
  if (request === null) {
    return failed("authorize.invalid");
  }
  // Only the request the page showed: another one posted to `/authorize/start` since then is refused.
  if (!sameAuthorizeRequest(request, field(form, "request"))) {
    return failed("authorize.outOfDate");
  }
  if (request.purpose === "link") {
    const view = readStatement(request.statement);
    if (view === null || !returnsToServer(request.returnTo, view.address)) {
      return failed("authorize.invalid");
    }
  }
  const session = await signedIn();
  if (session?.token === undefined) {
    return failed("authorize.signedOut");
  }
  const result = await answer(request, { client: session.client, token: session.token });
  if (!result.ok) {
    await leaveIfEnded(result.error);
    return refusal(result.error);
  }
  const fields: [string, string][] = [
    ["state", request.state],
    ["identity_token", result.value.identityToken],
    ["key_set", result.value.keySet],
  ];
  if (result.value.rootKey !== undefined) {
    fields.push(["root_key", result.value.rootKey]);
  }
  // The request stays until its cookie expires: deleting a cookie here would render the page again before the answer
  // is posted. Using it twice gets nothing new: a link statement works once at the API, and a second admin token is
  // for the same signed-in account.
  return { kind: "go", action: request.returnTo.href, fields };
}
