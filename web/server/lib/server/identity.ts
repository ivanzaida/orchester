/**
 * The name and email inside an Orchester Cloud identity token (a compact JWS), read for display only: *You're signed
 * in as Dana Ruiz (dana@example.com)* when the account has nothing to manage here. The team server is what verifies
 * the token; nothing here trusts what it reads.
 */
export interface Named {
  readonly name: string | null;
  readonly email: string | null;
}

export function namedIn(token: string): Named {
  const payload = token.split(".")[1];
  if (payload === undefined) {
    return { name: null, email: null };
  }
  try {
    const claims = JSON.parse(Buffer.from(payload, "base64url").toString("utf8")) as Record<string, unknown>;
    const text = (value: unknown) => (typeof value === "string" && value.length <= 254 ? value : null);
    return { name: text(claims.name), email: text(claims.email) };
  } catch {
    // Not a token this page can read: the page says it without a name.
    return { name: null, email: null };
  }
}
