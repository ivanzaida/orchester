import type { Metadata } from "next";
import { redirect } from "next/navigation";

import { Card, Head } from "@orchester/ui/components/card";
import { Shell } from "@/components/page";
import { host, readRequest, readStatement, requestQuery, returnsToServer, type AuthorizeRequest, type ServerView } from "@/lib/authorize";
import { t, type MessageKey } from "@/lib/i18n";
import { me } from "@/lib/server/cloud";
import { CloudUnavailableError } from "@/lib/server/failures";
import { linkedServer } from "@/lib/server/registry";
import { formToken, sessionEnded, signedIn } from "@/lib/server/visitor";

import { AuthorizeCard } from "./card";

export const metadata: Metadata = { title: t("authorize.pageTitle") };

type Search = Promise<Record<string, string | string[] | undefined>>;

function single(params: Record<string, string | string[] | undefined>): Record<string, string | undefined> {
  return Object.fromEntries(Object.entries(params).map(([name, value]) => [name, Array.isArray(value) ? undefined : value]));
}

/** An honest page for a request this site does not answer. */
function Refused({ reason }: { reason: MessageKey }) {
  return (
    <Shell>
      <Card>
        <Head title={t("authorize.refusedTitle")} description={t(reason)} tile={{ icon: "shield-alert", tone: "warning" }} />
      </Card>
    </Shell>
  );
}

function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter((word) => word !== "");
  return words.slice(0, 2).map((word) => Array.from(word)[0]?.toUpperCase() ?? "").join("");
}

/** The server the request is about: from the registry for a sign-in, from the statement (checked on confirming) for a link. */
async function serverOf(request: AuthorizeRequest, session: { token?: string | undefined }): Promise<ServerView | MessageKey> {
  if (request.purpose === "link") {
    return readStatement(request.statement) ?? "authorize.invalid";
  }
  const server = await linkedServer(request.server, session);
  if (server.ok) {
    return server.value;
  }
  if (server.error.kind === "refused" && server.error.code === "not_found") {
    return "authorize.notLinked";
  }
  throw new CloudUnavailableError(server.error);
}

/**
 * `/authorize` (`sssAw`, `WNc6d`): a team server's admin asks the cloud who the person is, or to link the server. The
 * person signs in first if needed, and comes back here.
 */
export default async function AuthorizePage({ searchParams }: { searchParams: Search }) {
  const request = readRequest(single(await searchParams));
  if (request === null) {
    return <Refused reason="authorize.invalid" />;
  }
  const back = `/authorize?${requestQuery(request)}`;
  const signIn = `/signin?next=${encodeURIComponent(back)}`;
  const session = await signedIn();
  if (session === null) {
    redirect(signIn);
  }
  const account = await me(session);
  if (!account.ok) {
    if (sessionEnded(account.error)) {
      redirect(signIn);
    }
    throw new CloudUnavailableError(account.error);
  }
  const server = await serverOf(request, session);
  if (typeof server === "string") {
    return <Refused reason={server} />;
  }
  if (!returnsToServer(request.returnTo, server.address)) {
    return <Refused reason="authorize.wrongReturn" />;
  }
  const own: [string, string] = request.purpose === "admin" ? ["server", request.server] : ["statement", request.statement];
  const fields: [string, string][] = [["purpose", request.purpose], ["state", request.state], ["return", request.returnTo.href], own];
  const { displayName: name, email } = account.value;
  return (
    <Shell>
      <AuthorizeCard csrf={await formToken()} purpose={request.purpose} request={fields}
        person={{ name, email, initials: initials(name) }} server={server} host={host(server.address)}
        notYou={signIn} cancel={`${request.returnTo.origin}/admin`} />
    </Shell>
  );
}
