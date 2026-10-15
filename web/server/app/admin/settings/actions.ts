"use server";

import { revalidatePath } from "next/cache";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { actor, failed, isMessage } from "@/lib/server/act";
import type { ApiResult, CallOptions } from "@/lib/server/api";
import { changeCloud, updateSettings } from "@/lib/server/team/settings";

export interface SettingState {
  /** Counts the answers, so an in-place form knows to close. */
  readonly answer: number;
  readonly done?: "pinned" | "unlinked";
  readonly error?: Message;
}

function run(op: string, form: FormData, options: CallOptions): Promise<ApiResult<unknown>> | Message {
  const on = field(form, "value") === "on";
  switch (op) {
    case "joinApproval":
      return updateSettings({ joinApproval: on }, options);
    case "accepting":
      return changeCloud({ accepting: on }, options);
    case "pin": {
      const key = field(form, "key").trim();
      return key === "" ? message("settings.pinEmpty") : changeCloud({ pin: key }, options);
    }
    case "unlink":
      return changeCloud({ unlink: true }, options);
    default:
      return message("error.refused");
  }
}

/** Settings' forms (`JzPl5`, `WV4AT`): join approval and cloud sign-ins (switches), pin another key, unlink. */
export async function settingAction(previous: SettingState, form: FormData): Promise<SettingState> {
  const answer = previous.answer + 1;
  const options = await actor(form);
  if (isMessage(options)) {
    return { answer, error: options };
  }
  const op = field(form, "op");
  const call = run(op, form, options);
  if ("key" in call) {
    return { answer, error: call };
  }
  const result = await call;
  if (!result.ok) {
    return { answer, error: await failed(result.error) };
  }
  revalidatePath("/admin", "layout");
  if (op === "pin" || op === "unlink") {
    return { answer, done: op === "pin" ? "pinned" : "unlinked" };
  }
  return { answer };
}
