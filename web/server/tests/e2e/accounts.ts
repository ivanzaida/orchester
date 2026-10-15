import { CLOUD_API, SERVER_API, type Stack } from "./stack";
import { waitFor } from "./processes";

/** People and computers the run makes through the APIs, so the browser only does what the admin pages do. */

export const PASSWORD = "correct horse battery staple";

async function json(response: Response): Promise<Record<string, unknown>> {
  return (await response.json()) as Record<string, unknown>;
}

async function post(url: string, body: unknown, token?: string): Promise<Response> {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (token !== undefined) {
    headers.authorization = `Bearer ${token}`;
  }
  return fetch(url, { method: "POST", headers, body: JSON.stringify(body) });
}

/** An Orchester Cloud account, signed up and confirmed through the link the cloud emailed. */
export async function cloudAccount(stack: Stack, email: string, name: string): Promise<void> {
  const signUp = await post(`${CLOUD_API}/auth/signup`, { email, password: PASSWORD, display_name: name });
  if (signUp.status !== 202) {
    throw new Error(`sign-up answered ${String(signUp.status)}`);
  }
  const token = await waitFor("the confirmation email", () => {
    const body = stack.smtp.to(email).at(-1)?.body ?? "";
    return /\/confirm\/([0-9a-f]{64})/.exec(body)?.[1] ?? null;
  });
  const confirmed = await post(`${CLOUD_API}/auth/confirm`, { link_token: token, password: PASSWORD });
  if (confirmed.status !== 204) {
    throw new Error(`confirmation answered ${String(confirmed.status)}`);
  }
}

/** A desktop's cloud device token for `email`. */
export async function cloudDevice(email: string): Promise<string> {
  const response = await post(`${CLOUD_API}/auth/sessions`, { email, password: PASSWORD,
    device: { name: "Test computer", platform: "windows" } });
  const body = await json(response);
  if (typeof body.token !== "string") {
    throw new Error(`cloud sign-in answered ${String(response.status)}`);
  }
  return body.token;
}

/** The invites the cloud lists to the account of `device`. */
export async function cloudInvites(device: string): Promise<Record<string, unknown>[]> {
  const response = await fetch(`${CLOUD_API}/invites`, { headers: { authorization: `Bearer ${device}` } });
  const body = await json(response);
  return Array.isArray(body.invites) ? (body.invites as Record<string, unknown>[]) : [];
}

/** A computer joining the team server with an invite code, as Orchester does: its device token. */
export async function joinWithCode(code: string, username: string, device: string): Promise<string> {
  const response = await post(`${SERVER_API}/auth/code`, { code, device, username, display_name: username });
  const body = await json(response);
  if (typeof body.token !== "string") {
    throw new Error(`joining with a code answered ${String(response.status)}`);
  }
  return body.token;
}

/** Whether the team server still takes `token`. */
export async function tokenWorks(token: string): Promise<boolean> {
  const response = await fetch(`${SERVER_API}/auth/me`, { headers: { authorization: `Bearer ${token}` } });
  return response.status === 200;
}
