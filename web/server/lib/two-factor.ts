import type { TwoFactorStanding } from "@/lib/model";

/**
 * What *Invite by email* says about two-factor authentication, which sending needs: nothing while this sign-in may
 * send; *Sign in again* when it signed in without it, as the account may have turned it on since; *Turn it on* only
 * once a sign-in again came back without it. `refused`: the server just answered `403 mfa_required`, which a sign-in
 * from before the admin kept its standing learns only so.
 */
export type TwoFactorNotice = "none" | "again" | "turnOn";

export function twoFactorNotice(standing: TwoFactorStanding, refused: boolean): TwoFactorNotice {
  switch (standing) {
    case "off":
      return "turnOn";
    case "without":
      return "again";
    case "on":
    case "unknown":
      return refused ? "again" : "none";
  }
}
