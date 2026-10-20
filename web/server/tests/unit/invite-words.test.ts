import { describe, expect, it } from "vitest";

import { t } from "@/lib/i18n";
import type { TwoFactorStanding } from "@/lib/model";
import { repeatedError } from "@/lib/server/repeated";
import { twoFactorNotice } from "@/lib/two-factor";

/** What *Invite by email* says when the server refuses a repeated invite, and about two-factor authentication. */

describe("a repeated invite by email", () => {
  const sentAt = Date.UTC(2026, 9, 20, 20, 1);

  it("names the open invite and when it was sent, in the email field", () => {
    const error = repeatedError({ kind: "refused", status: 409, code: "invite_open", sentAt }, "slyapa@tuxick.dev", "project");
    expect(error).toEqual({ key: "invites.openInvite", params: { email: "slyapa@tuxick.dev" }, when: sentAt });
    expect(t("invites.openInvite", { email: "slyapa@tuxick.dev", when: "Oct 20, 20:01" })).toBe(
      "slyapa@tuxick.dev already has an open invite, sent Oct 20, 20:01. Revoke it to send a new one.",
    );
  });

  it("says a member is one already, or that the address has an account on the server", () => {
    const member = { kind: "refused" as const, status: 409, code: "already_member" as const };
    expect(repeatedError(member, "bob@example.com", "project")).toEqual({ key: "invites.alreadyMember", params: { email: "bob@example.com" } });
    expect(repeatedError(member, "bob@example.com", "server")).toEqual({ key: "invites.alreadyAccount", params: { email: "bob@example.com" } });
  });

  it("leaves every other failure to the general words", () => {
    expect(repeatedError({ kind: "unreachable" }, "a@example.com", "project")).toBeNull();
    expect(repeatedError({ kind: "refused", status: 409, code: "conflict" }, "a@example.com", "project")).toBeNull();
    expect(repeatedError({ kind: "refused", status: 409, code: "invite_open" }, "a@example.com", "project")).toBeNull();
  });
});

describe("the two-factor notice", () => {
  const cases: [TwoFactorStanding, boolean, ReturnType<typeof twoFactorNotice>][] = [
    ["on", false, "none"],
    ["unknown", false, "none"],
    // Signed in before turning it on, or without it: told so on opening the page, and offered to sign in again.
    ["without", false, "again"],
    ["without", true, "again"],
    // A sign-in from before the admin kept its standing learns it from the server's refusal.
    ["unknown", true, "again"],
    // Only after signing in again came back without it is the account one without: where to turn it on.
    ["off", false, "turnOn"],
    ["off", true, "turnOn"],
  ];

  it.each(cases)("for a sign-in %s, refused %s, is %s", (standing, refused, notice) => {
    expect(twoFactorNotice(standing, refused)).toBe(notice);
  });
});
