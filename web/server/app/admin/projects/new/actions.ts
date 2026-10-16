"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import { loadServer } from "@/lib/server/admin";
import { createProject } from "@/lib/server/team/projects";
import { hintError, projectNameError, slugError } from "@/lib/validation";

type ProjectField = "name" | "slug" | "hint";

export type NewProjectState =
  | {
      readonly view: "form";
      readonly answer: number;
      readonly values: Readonly<Record<ProjectField, string>>;
      readonly errors: Partial<Record<ProjectField, Message>>;
      readonly error?: Message;
    }
  | { readonly view: "created"; readonly answer: number; readonly name: string; readonly slug: string; readonly address: string };

function check(values: Record<ProjectField, string>): Partial<Record<ProjectField, Message>> {
  const found: Partial<Record<ProjectField, Message>> = {};
  const name = projectNameError(values.name);
  const slug = slugError(values.slug);
  const hint = hintError(values.hint);
  if (name !== undefined) {
    found.name = name;
  }
  if (slug !== undefined) {
    found.slug = slug;
  }
  if (hint !== undefined) {
    found.hint = hint;
  }
  return found;
}

/**
 * *Create project* (`gv7xE`; slug taken or not allowed, created in `KagFB`). The idempotency key comes with the page,
 * so a form sent twice makes one project.
 */
export async function newProjectAction(previous: NewProjectState, form: FormData): Promise<NewProjectState> {
  const answer = previous.answer + 1;
  const values = { name: field(form, "name").trim(), slug: field(form, "slug").trim(), hint: field(form, "hint").trim() };
  const options = await actor(form);
  if (isMessage(options)) {
    return { view: "form", answer, values, errors: {}, error: options };
  }
  const errors = check(values);
  if (Object.keys(errors).length > 0) {
    return { view: "form", answer, values, errors };
  }
  const input = { idempotencyKey: field(form, "key"), name: values.name, slug: values.slug, remoteHint: values.hint === "" ? null : values.hint };
  const result = await createProject(input, options);
  if (!result.ok) {
    if (result.error.kind === "refused" && result.error.code === "slug_taken") {
      const server = (await loadServer()).name;
      return { view: "form", answer, values, errors: { slug: message("newProject.slugTaken", { server }) } };
    }
    return { view: "form", answer, values, errors: {}, error: await failed(result.error) };
  }
  revalidatePath("/admin", "layout");
  return { view: "created", answer, name: values.name, slug: result.value.slug, address: result.value.address };
}
