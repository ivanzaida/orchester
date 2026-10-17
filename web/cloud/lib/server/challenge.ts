import { cookies } from "next/headers";

import { config } from "@/lib/server/config";
import { CHALLENGE_COOKIE, CHALLENGE_SECONDS, cookieOptions } from "@/lib/server/cookie-options";
import { deriveKey } from "@/lib/server/seal";
import { openChallenge, sealChallenge, type Challenge } from "@/lib/server/session";

/**
 * The sign-in waiting for its code (`/signin/code`), in this browser's sealed cookie (`session.ts` seals it). The
 * browser never reads the challenge, as it never reads the session token.
 */
const key = () => deriveKey(config().secret, "challenge");

/** Keeps the password step's challenge in this browser, sealed, for the five minutes the cloud gives it. */
export async function keepChallenge(challenge: Challenge): Promise<void> {
  (await cookies()).set(CHALLENGE_COOKIE, sealChallenge(key(), challenge, Date.now()), cookieOptions(CHALLENGE_SECONDS));
}

/** The challenge this browser kept, while it is fresh. */
export async function waitingChallenge(): Promise<Challenge | null> {
  return openChallenge(key(), (await cookies()).get(CHALLENGE_COOKIE)?.value, Date.now());
}

/** Forgets the challenge once it is used or ended. Only a cookie that exists is deleted, as for the session. */
export async function forgetChallenge(): Promise<void> {
  const jar = await cookies();
  if (jar.has(CHALLENGE_COOKIE)) {
    jar.delete({ name: CHALLENGE_COOKIE, ...cookieOptions(0) });
  }
}
