import { describe, expect, it } from "vitest";

import { csrfToken, originMatches, tokenMatches } from "@/lib/server/csrf";
import { newPresession, presessionLive } from "@/lib/server/presession";
import { deriveKey } from "@/lib/server/seal";

const key = deriveKey(new Uint8Array(32).fill(1), "csrf");
const other = deriveKey(new Uint8Array(32).fill(2), "csrf");
const abc = newPresession(Date.now());
const abd = newPresession(Date.now());

describe("csrf token", () => {
  it("is the same for the same pre-session and refused for another", () => {
    const token = csrfToken(key, { presession: abc });
    expect(token).not.toBeNull();
    expect(tokenMatches(key, { presession: abc }, token)).toBe(true);
    expect(tokenMatches(key, { presession: abd }, token)).toBe(false);
  });

  it("binds to the session once there is one, not to the pre-session", () => {
    const before = csrfToken(key, { presession: abc });
    const after = csrfToken(key, { presession: abc, session: "sealed" });
    expect(after).not.toEqual(before);
    expect(tokenMatches(key, { presession: abc, session: "sealed" }, before)).toBe(false);
    expect(tokenMatches(key, { presession: abc, session: "sealed" }, after)).toBe(true);
  });

  it("does not contain the cookie it is bound to", () => {
    const token = csrfToken(key, { presession: abc });
    expect(token).not.toContain(abc.split(".")[0]);
  });

  it("is refused when made with another secret, missing or not text", () => {
    const token = csrfToken(other, { presession: abc });
    expect(tokenMatches(key, { presession: abc }, token)).toBe(false);
    expect(tokenMatches(key, { presession: abc }, null)).toBe(false);
    expect(tokenMatches(key, { presession: abc }, new File([], "x"))).toBe(false);
  });

  it("cannot be made or checked without either cookie", () => {
    expect(csrfToken(key, {})).toBeNull();
    expect(tokenMatches(key, {}, "anything")).toBe(false);
  });
});

describe("pre-session cookie", () => {
  const NOW = 1_800_000_000_000;

  it("carries its own expiry, and its form token stops working with it", () => {
    const value = newPresession(NOW);
    expect(presessionLive(value, NOW + 1000)).toBe(true);
    expect(presessionLive(value, NOW + 2 * 60 * 60 * 1000)).toBe(false);
    const token = csrfToken(key, { presession: value }, NOW);
    expect(token).not.toBeNull();
    expect(tokenMatches(key, { presession: value }, token, NOW + 1000)).toBe(true);
    expect(tokenMatches(key, { presession: value }, token, NOW + 2 * 60 * 60 * 1000)).toBe(false);
    expect(csrfToken(key, { presession: value }, NOW + 2 * 60 * 60 * 1000)).toBeNull();
  });

  it("whose expiry was changed no longer matches its token", () => {
    const value = newPresession(NOW);
    const token = csrfToken(key, { presession: value }, NOW);
    const [random] = value.split(".");
    const later = `${random ?? ""}.${NOW + 10 * 60 * 60 * 1000}`;
    expect(tokenMatches(key, { presession: later }, token, NOW + 3 * 60 * 60 * 1000)).toBe(false);
  });

  it("without an expiry is refused", () => {
    expect(presessionLive("only-random", NOW)).toBe(false);
  });
});

describe("origin", () => {
  const site = ["https://orchester.cloud"];
  const moving = ["https://orchester.cloud", "https://orchester-cloud.lurq.dev"];

  it("must be exactly the public origin", () => {
    expect(originMatches("https://orchester.cloud", site)).toBe(true);
    expect(originMatches("https://evil.example", site)).toBe(false);
    expect(originMatches("http://orchester.cloud", site)).toBe(false);
    expect(originMatches("https://orchester.cloud:8443", site)).toBe(false);
    expect(originMatches("https://www.orchester.cloud", site)).toBe(false);
    expect(originMatches("null", site)).toBe(false);
    expect(originMatches("https://orchester-cloud.lurq.dev", site)).toBe(false);
  });

  it("may be one of the extra origins while the site moves", () => {
    expect(originMatches("https://orchester.cloud", moving)).toBe(true);
    expect(originMatches("https://orchester-cloud.lurq.dev", moving)).toBe(true);
    expect(originMatches("http://orchester-cloud.lurq.dev", moving)).toBe(false);
    expect(originMatches("https://orchester-cloud.lurq.dev.evil.example", moving)).toBe(false);
    expect(originMatches("https://evil.example", moving)).toBe(false);
    expect(originMatches("null", moving)).toBe(false);
  });

  it("is refused when the browser sent none", () => {
    expect(originMatches(null, site)).toBe(false);
    expect(originMatches(null, moving)).toBe(false);
  });
});
