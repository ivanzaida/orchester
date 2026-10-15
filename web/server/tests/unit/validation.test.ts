import { describe, expect, it } from "vitest";

import { bareAddress, initials, shortFingerprint } from "@/lib/codes";
import { codeError, emailError, normalizeEmail, projectNameError, slugError, slugFromName, usernameError } from "@/lib/validation";

describe("slug", () => {
  it("follows core's Slug: small letters, digits and single hyphens", () => {
    expect(slugError("kontur")).toBeUndefined();
    expect(slugError("api-gateway")).toBeUndefined();
    expect(slugError("API Gateway")).toEqual({ key: "newProject.slugShapeLike", params: { slug: "api-gateway" } });
    expect(slugError("-x")).toEqual({ key: "newProject.slugShapeLike", params: { slug: "x" } });
    expect(slugError("Ä")).toEqual({ key: "newProject.slugShape" });
    expect(slugError("a".repeat(65))).toBeDefined();
  });

  it("refuses the names the server keeps", () => {
    for (const slug of ["admin", "api", "auth", "_next", ".well-known"]) {
      expect(slugError(slug), slug).toEqual({ key: "newProject.slugReserved", params: { slug } });
    }
  });

  it("is made from a name like core's from_name", () => {
    expect(slugFromName("Mobile app")).toBe("mobile-app");
    expect(slugFromName("  API — Gateway!! ")).toBe("api-gateway");
    expect(slugFromName("Тест")).toBe("");
  });
});

describe("names and codes", () => {
  it("need a name of the allowed length", () => {
    expect(projectNameError("  ")).toEqual({ key: "error.nameEmpty" });
    expect(projectNameError("x".repeat(201))).toEqual({ key: "error.nameLong", params: { count: 201, max: 200 } });
  });

  it("need a username of small letters, digits and hyphens", () => {
    expect(usernameError("ivan")).toBeUndefined();
    expect(usernameError("Ivan")).toBeDefined();
    expect(usernameError("ivan zaida")).toBeDefined();
  });

  it("take a code with or without its hyphen, in any case", () => {
    expect(codeError("h8rw-2nce")).toBeUndefined();
    expect(codeError("H8RW 2NCE")).toBeUndefined();
    expect(codeError("H8RW-2NC")).toEqual({ key: "error.codeShape" });
  });

  it("keep an email trimmed and lowercased, and refuse one without a dotted domain", () => {
    expect(normalizeEmail("  Dana@Example.COM ")).toBe("dana@example.com");
    expect(emailError("dana@example")).toEqual({ key: "error.emailInvalid" });
    expect(emailError("a@b@example.com")).toBeDefined();
  });
});

describe("raw data", () => {
  it("writes a fingerprint as the browser shows it, shortened as the design does", () => {
    const sha = "4f7a92c10d8e51b3a91c7e2d4b6f8a0c1e3d5f7092b4c6d8e0f1a2b36a2fc091";
    expect(shortFingerprint(sha)).toBe("4F:7A:92:C1:0D:8E:51:B3 … 6A:2F:C0:91");
  });

  it("makes initials and bare addresses", () => {
    expect(initials("Ivan Zaida")).toBe("IZ");
    expect(initials("maria")).toBe("M");
    expect(bareAddress("https://orchester.kontur.dev/kontur")).toBe("orchester.kontur.dev/kontur");
  });
});
