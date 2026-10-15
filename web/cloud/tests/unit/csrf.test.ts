import { describe, expect, it } from "vitest";

import { csrfToken, originMatches, tokenMatches } from "@/lib/server/csrf";
import { deriveKey } from "@/lib/server/seal";

const key = deriveKey(new Uint8Array(32).fill(1), "csrf");
const other = deriveKey(new Uint8Array(32).fill(2), "csrf");

describe("csrf token", () => {
  it("is the same for the same pre-session and refused for another", () => {
    const token = csrfToken(key, { presession: "abc" });
    expect(token).not.toBeNull();
    expect(tokenMatches(key, { presession: "abc" }, token)).toBe(true);
    expect(tokenMatches(key, { presession: "abd" }, token)).toBe(false);
  });

  it("binds to the session once there is one, not to the pre-session", () => {
    const before = csrfToken(key, { presession: "abc" });
    const after = csrfToken(key, { presession: "abc", session: "sealed" });
    expect(after).not.toEqual(before);
    expect(tokenMatches(key, { presession: "abc", session: "sealed" }, before)).toBe(false);
    expect(tokenMatches(key, { presession: "abc", session: "sealed" }, after)).toBe(true);
  });

  it("does not contain the cookie it is bound to", () => {
    const token = csrfToken(key, { presession: "visible-cookie-value" });
    expect(token).not.toContain("visible-cookie-value");
  });

  it("is refused when made with another secret, missing or not text", () => {
    const token = csrfToken(other, { presession: "abc" });
    expect(tokenMatches(key, { presession: "abc" }, token)).toBe(false);
    expect(tokenMatches(key, { presession: "abc" }, null)).toBe(false);
    expect(tokenMatches(key, { presession: "abc" }, new File([], "x"))).toBe(false);
  });

  it("cannot be made or checked without either cookie", () => {
    expect(csrfToken(key, {})).toBeNull();
    expect(tokenMatches(key, {}, "anything")).toBe(false);
  });
});

describe("origin", () => {
  it("must be exactly the public origin", () => {
    expect(originMatches("https://orchester-cloud.lurq.dev", "https://orchester-cloud.lurq.dev")).toBe(true);
    expect(originMatches("https://evil.example", "https://orchester-cloud.lurq.dev")).toBe(false);
    expect(originMatches("http://orchester-cloud.lurq.dev", "https://orchester-cloud.lurq.dev")).toBe(false);
    expect(originMatches("null", "https://orchester-cloud.lurq.dev")).toBe(false);
  });

  it("is refused when the browser sent none", () => {
    expect(originMatches(null, "https://orchester-cloud.lurq.dev")).toBe(false);
  });
});
