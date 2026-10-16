import { redirect } from "next/navigation";
import { cache } from "react";

import type { Device, Person, Project, ServerInfo, User } from "@/lib/model";
import type { ApiResult, CallOptions } from "@/lib/server/api";
import { ServerUnavailableError, sessionEnded } from "@/lib/server/failures";
import type { Place } from "@/lib/server/place";
import type { SignedInWith } from "@/lib/server/session";
import { me, serverInfo } from "@/lib/server/team/auth";
import { devices, people } from "@/lib/server/team/people";
import { managedProjects } from "@/lib/server/team/projects";
import { anonymous, session } from "@/lib/server/visitor";

/**
 * Everything the signed-in admin pages share, read once per request: the server, the person, how they signed in,
 * and the lists the navigation counts. Owners who are not admins get no people or devices.
 */
export interface AdminContext {
  readonly server: ServerInfo;
  readonly me: User;
  readonly via: SignedInWith;
  readonly options: CallOptions;
  readonly place: Place;
  readonly projects: readonly Project[];
  readonly people: readonly Person[] | null;
  readonly devices: readonly Device[] | null;
}

/** A page cannot change cookies: this route forgets a session the server ended, then sends the person to sign in. */
export const SESSION_ENDED = "/admin/session-ended";

/** A page's own call: its value, or the session-ended route, or the error page. */
export function value<T>(result: ApiResult<T>): T {
  if (!result.ok) {
    if (sessionEnded(result.error)) {
      redirect(SESSION_ENDED);
    }
    throw new ServerUnavailableError(result.error);
  }
  return result.value;
}

/** The server's description, or the error page when it does not answer. */
export const loadServer = cache(async (): Promise<ServerInfo> => {
  const result = await serverInfo(await anonymous());
  if (!result.ok) {
    throw new ServerUnavailableError(result.error);
  }
  return result.value;
});

async function adminLists(options: CallOptions, admin: boolean) {
  if (!admin) {
    return { people: null, devices: null };
  }
  const [peopleList, deviceList] = await Promise.all([people(options), devices(options)]);
  return { people: value(peopleList), devices: value(deviceList) };
}

export const loadAdmin = cache(async (): Promise<AdminContext> => {
  const server = await loadServer();
  if (server.needsSetup) {
    redirect("/admin/setup");
  }
  const current = await session();
  if (current === null) {
    redirect("/admin/signin");
  }
  const options = { ...(await anonymous()), token: current.token };
  const [user, projects] = await Promise.all([me(options), managedProjects(options)]);
  const person = value(user);
  const lists = await adminLists(options, person.admin);
  return { server, me: person, via: current.via, options, place: options.place, projects: value(projects), ...lists };
});

/** The project a page address names by its slug, among those this person manages. */
export async function loadProject(slug: string): Promise<{ context: AdminContext; project: Project | null }> {
  const context = await loadAdmin();
  return { context, project: context.projects.find((project) => project.slug === slug) ?? null };
}
