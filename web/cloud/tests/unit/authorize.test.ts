import { describe, expect, it } from "vitest";

import {
  adminReturn,
  callbackUrl,
  checkCode,
  nextAfterSignIn,
  readRequest,
  readStatement,
  requestQuery,
  returnsToServer,
} from "@/lib/authorize";

const SERVER = "0199f0a0-0000-7000-8000-00000000c10d";
const STATE = "aB3_-aB3_-aB3_-aB3_-aB3_-aB3_-aB3_-aB3_-aB3";
const RETURN = "https://orchester.kontur.test/admin/auth/callback";

function statement(payload: Record<string, unknown>): string {
  const part = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
  return `${part({ alg: "EdDSA", typ: "orchester-link+jws" })}.${part(payload)}.c2lnbmF0dXJl`;
}

describe("the check code", () => {
  it("is the first 80 bits of the fingerprint in Crockford groups, as the Rust side makes it", () => {
    expect(checkCode("ab".repeat(32))).toBe("NENT-QAXB-NENT-QAXB");
    expect(checkCode("00".repeat(32))).toBe("0000-0000-0000-0000");
  });

  it("is none for anything but 64 lowercase hex characters", () => {
    expect(checkCode("AB".repeat(32))).toBeNull();
    expect(checkCode("ab".repeat(31))).toBeNull();
  });
});

describe("an authorize request", () => {
  it("names the callback on the server's own address and nothing more", () => {
    expect(callbackUrl(RETURN)?.origin).toBe("https://orchester.kontur.test");
    expect(callbackUrl("http://127.0.0.1:4440/admin/auth/callback")).not.toBeNull();
    for (const bad of ["http://orchester.kontur.test/admin/auth/callback", `${RETURN}?next=x`, `${RETURN}#x`,
      "https://orchester.kontur.test/elsewhere", "https://user:pw@orchester.kontur.test/admin/auth/callback", "nope"]) {
      expect(callbackUrl(bad), bad).toBeNull();
    }
  });

  it("is a sign-in with a server id or a link with a statement, and survives the trip through sign-in", () => {
    const admin = readRequest({ purpose: "admin", server: SERVER, state: STATE, return: RETURN });
    expect(admin?.purpose).toBe("admin");
    const link = readRequest({ purpose: "link", statement: "a.b.c", state: STATE, return: RETURN });
    expect(link?.purpose).toBe("link");
    if (admin === null) {
      throw new Error("a sign-in request");
    }
    const again = readRequest(Object.fromEntries(new URLSearchParams(requestQuery(admin))));
    expect(again).toEqual(admin);
  });

  it("refuses what is missing or malformed", () => {
    expect(readRequest({ purpose: "admin", server: "not-a-uuid", state: STATE, return: RETURN })).toBeNull();
    expect(readRequest({ purpose: "admin", server: SERVER, state: "short", return: RETURN })).toBeNull();
    expect(readRequest({ purpose: "link", statement: "no dots", state: STATE, return: RETURN })).toBeNull();
    expect(readRequest({ purpose: "other", server: SERVER, state: STATE, return: RETURN })).toBeNull();
  });

  it("goes back only to the server it is about", () => {
    const url = new URL(RETURN);
    expect(returnsToServer(url, "https://orchester.kontur.test")).toBe(true);
    expect(returnsToServer(url, "https://evil.example")).toBe(false);
    expect(returnsToServer(url, "https://orchester.kontur.test:8740")).toBe(false);
  });
});

describe("a link statement", () => {
  it("is read for display, with its check code", () => {
    const view = readStatement(statement({ name: " Kontur team ", address: "https://orchester.kontur.test",
      fingerprint: "ab".repeat(32) }));
    expect(view).toEqual({ name: "Kontur team", address: "https://orchester.kontur.test", checkCode: "NENT-QAXB-NENT-QAXB" });
  });

  it("without a name or address is none", () => {
    expect(readStatement(statement({ address: "https://x.test" }))).toBeNull();
    expect(readStatement("a.%%%.c")).toBeNull();
  });
});

describe("where the pages lead", () => {
  it("sign-in comes back only to /authorize", () => {
    expect(nextAfterSignIn("/authorize?purpose=admin")).toBe("/authorize?purpose=admin");
    for (const bad of ["/account", "//evil.example/authorize?", "https://evil.example/authorize?", undefined]) {
      expect(nextAfterSignIn(bad), String(bad)).toBeNull();
    }
  });

  it("a delivered invite leads back only to a server admin's page", () => {
    expect(adminReturn("https://orchester.kontur.test/admin/projects/kontur/invites")).not.toBeNull();
    for (const bad of ["https://orchester.kontur.test/elsewhere", "http://orchester.kontur.test/admin", "javascript:alert(1)"]) {
      expect(adminReturn(bad), bad).toBeNull();
    }
  });
});
