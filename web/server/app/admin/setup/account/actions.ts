"use server";

import { headers } from "next/headers";
import { redirect } from "next/navigation";

import { field } from "@/lib/form";
import { message, type Message } from "@/lib/i18n";
import { browserDevice } from "@/lib/server/device";
import { failureMessage } from "@/lib/server/failures";
import { signedInStanding } from "@/lib/server/session";
import { signInWithCode } from "@/lib/server/team/auth";
import { updateSettings } from "@/lib/server/team/settings";
import { anonymous, forgetSetupCode, formIsGenuine, setupCode, startSession } from "@/lib/server/visitor";
import { displayNameError, serverNameError, usernameError } from "@/lib/validation";

type AccountField = "serverName" | "username" | "displayName";

export interface AccountState {
  readonly attempt: number;
  readonly values: Readonly<Record<AccountField, string>>;
  readonly errors: Partial<Record<AccountField, Message>>;
  readonly error?: Message;
}

function check(values: Record<AccountField, string>): Partial<Record<AccountField, Message>> {
  const found: Partial<Record<AccountField, Message>> = {};
  const checks: [AccountField, Message | undefined][] = [
    ["serverName", serverNameError(values.serverName)],
    ["username", usernameError(values.username)],
    ["displayName", displayNameError(values.displayName)],
  ];
  for (const [name, error] of checks) {
    if (error !== undefined) {
      found[name] = error;
    }
  }
  return found;
}

/**
 * Setup step 2: the setup code with a username and display name makes the first admin, signed in here as a browser.
 * The server's name is saved with the new session; if that fails the account still stands, and the next page says so.
 */
export async function accountAction(previous: AccountState, form: FormData): Promise<AccountState> {
  const values = { serverName: field(form, "serverName").trim(), username: field(form, "username").trim(), displayName: field(form, "displayName").trim() };
  const attempt = previous.attempt + 1;
  if (!(await formIsGenuine(form))) {
    return { attempt, values, errors: {}, error: message("error.formExpired") };
  }
  const errors = check(values);
  if (Object.keys(errors).length > 0) {
    return { attempt, values, errors };
  }
  const code = await setupCode();
  if (code === null) {
    redirect("/admin/setup?expired=1");
  }
  const device = browserDevice((await headers()).get("user-agent"));
  const account = { username: values.username, displayName: values.displayName };
  const result = await signInWithCode({ code, device, account }, await anonymous());
  if (!result.ok) {
    const failure = result.error;
    if (failure.kind === "refused" && failure.code === "invalid_code") {
      await forgetSetupCode();
      redirect("/admin/setup?expired=1");
    }
    if (failure.kind === "refused" && failure.code === "username_taken") {
      return { attempt, values, errors: { username: message("setup.usernameTaken") } };
    }
    return { attempt, values, errors: {}, error: failureMessage(failure, Date.now()) };
  }
  await forgetSetupCode();
  await startSession({ token: result.value.token, via: "code", twoFactor: signedInStanding(result.value.mfa, false) });
  const named = await updateSettings({ name: values.serverName }, { ...(await anonymous()), token: result.value.token });
  redirect(named.ok ? "/admin/setup/cloud" : "/admin/setup/cloud?name=unsaved");
}
