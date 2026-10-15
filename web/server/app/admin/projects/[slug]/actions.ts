"use server";

import { revalidatePath } from "next/cache";
import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { removeMember, setRole } from "@/lib/server/team/projects";

export interface MemberState {
  /** The server refused: the project would have no owner left (`last_owner`). */
  readonly lastOwner?: boolean;
  readonly removed?: boolean;
  readonly error?: Message;
}

/**
 * A member row's two forms: `op=role` changes the role, `op=remove` removes the member, or the person themself
 * (*Leave*). Removing or leaving was confirmed in place before this runs.
 */
export async function memberAction(_: MemberState, form: FormData): Promise<MemberState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const project = field(form, "project");
  const user = field(form, "user");
  const role = field(form, "role");
  const remove = field(form, "op") === "remove";
  if (!remove && role !== "owner" && role !== "member") {
    return { error: message("error.refused") };
  }
  const result = remove
    ? await removeMember(project, user, options)
    : await setRole(project, user, role === "owner" ? "owner" : "member", options);
  if (!result.ok) {
    if (result.error.kind === "refused" && result.error.code === "last_owner") {
      return { lastOwner: true };
    }
    return { error: await failed(result.error) };
  }
  if (remove && field(form, "self") === "yes") {
    redirect("/admin");
  }
  if (remove) {
    // The row stays, replaced by what happened to it (`KagFB`); the list is read again on the next visit.
    return { removed: true };
  }
  revalidatePath("/admin", "layout");
  return {};
}
