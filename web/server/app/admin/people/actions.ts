"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { updatePerson } from "@/lib/server/team/people";

export interface PersonState {
  readonly lastAdmin?: boolean;
  readonly error?: Message;
}

const CHANGES = {
  admin: { admin: true },
  disable: { disabled: true },
  enable: { disabled: false },
} as const;

/** *Make admin* and *Disable* (both confirmed in place), *Enable* (`waaNU`, `Gs9Vu`). */
export async function personAction(_: PersonState, form: FormData): Promise<PersonState> {
  const options = await actor(form);
  if (isMessage(options)) {
    return { error: options };
  }
  const op = field(form, "op");
  if (op !== "admin" && op !== "disable" && op !== "enable") {
    return { error: message("error.refused") };
  }
  const result = await updatePerson(field(form, "user"), CHANGES[op], options);
  if (!result.ok) {
    if (result.error.kind === "refused" && result.error.code === "last_admin") {
      return { lastAdmin: true };
    }
    return { error: await failed(result.error) };
  }
  revalidatePath("/admin", "layout");
  return {};
}
