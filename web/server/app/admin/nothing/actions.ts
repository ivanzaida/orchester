"use server";

import { redirect } from "next/navigation";

import { endNothingFlow } from "@/lib/server/visitor";

/** *Sign out* on *Nothing to manage here*: the server issued no session, so only the page's own cookie goes. */
export async function leaveAction(): Promise<never> {
  await endNothingFlow();
  redirect("/admin/signin");
}
