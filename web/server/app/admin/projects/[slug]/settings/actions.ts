"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import type { Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { updateProject } from "@/lib/server/team/projects";
import { hintError, projectNameError } from "@/lib/validation";

export interface SaveState {
  /** Counts the answers, so the row knows a new one came. */
  readonly answer: number;
  readonly saved: boolean;
  readonly error?: Message;
}

/** *Save* on a project's Name or Repository hint (`h8OLW5`); an empty hint clears it. */
export async function saveProjectAction(previous: SaveState, form: FormData): Promise<SaveState> {
  const answer = previous.answer + 1;
  const options = await actor(form);
  if (isMessage(options)) {
    return { answer, saved: false, error: options };
  }
  const which = field(form, "which");
  const value = field(form, "value").trim();
  const invalid = which === "name" ? projectNameError(value) : hintError(value);
  if (invalid !== undefined) {
    return { answer, saved: false, error: invalid };
  }
  const changes = which === "name" ? { name: value } : { remoteHint: value };
  const result = await updateProject(field(form, "project"), changes, options);
  if (!result.ok) {
    return { answer, saved: false, error: await failed(result.error) };
  }
  revalidatePath("/admin", "layout");
  return { answer, saved: true };
}
