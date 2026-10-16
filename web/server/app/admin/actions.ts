"use server";

import { redirect } from "next/navigation";

import type { Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { sessionEnded } from "@/lib/server/failures";
import { signOut } from "@/lib/server/team/auth";
import { endSession } from "@/lib/server/visitor";

export interface FormState {
  readonly error?: Message;
}

/**
 * Ends this browser's admin session on the server, then forgets it here. When the server cannot be told, the session
 * stays and the person reads why: a cookie dropped here while its token still works would only look signed out.
 */
export async function signOutAction(_: FormState, form: FormData): Promise<FormState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const result = await signOut(options);
  if (!result.ok && !sessionEnded(result.error)) {
    return { error: await failed(result.error) };
  }
  await endSession();
  redirect("/admin/signin");
}
