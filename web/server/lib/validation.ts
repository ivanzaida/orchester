import { message, type Message } from "@/lib/i18n";

/**
 * The team server's input rules, checked in the browser before sending and again on the admin's server side; the
 * server checks once more. Slugs follow core's `Slug` (1–64 small letters, digits and single hyphens), names core's
 * `Name` (200 characters), server names the server's settings (128), display names its accounts (256).
 */
export const SLUG_MAX = 64;
const PROJECT_NAME_MAX = 200;
const SERVER_NAME_MAX = 128;
const DISPLAY_NAME_MAX = 256;
const USERNAME_MAX = 64;
const EMAIL_MAX = 254;

/** Slugs the server keeps for itself (`docs/architecture/server.md`, "Servers and project addresses"). */
const RESERVED = new Set(["admin", "api", "auth"]);

function length(value: string): number {
  // Unicode scalar values, as the server counts (`chars().count()`).
  return Array.from(value).length;
}

export function slugShapeOk(slug: string): boolean {
  return /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug) && slug.length <= SLUG_MAX;
}

/** The slug a name reads as, like core's `Slug::from_name`: *API Gateway* → `api-gateway`. */
export function slugFromName(name: string): string {
  const slug = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, SLUG_MAX);
  return slug.replace(/-+$/, "");
}

export function slugError(slug: string): Message | undefined {
  if (RESERVED.has(slug) || slug.startsWith("_") || slug.startsWith(".")) {
    return message("newProject.slugReserved", { slug });
  }
  if (slugShapeOk(slug)) {
    return undefined;
  }
  const suggestion = slugFromName(slug);
  return suggestion === "" ? message("newProject.slugShape") : message("newProject.slugShapeLike", { slug: suggestion });
}

function named(value: string, max: number): Message | undefined {
  const count = length(value.trim());
  if (count === 0) {
    return message("error.nameEmpty");
  }
  return count > max ? message("error.nameLong", { count, max }) : undefined;
}

export function projectNameError(value: string): Message | undefined {
  return named(value, PROJECT_NAME_MAX);
}

export function serverNameError(value: string): Message | undefined {
  return named(value, SERVER_NAME_MAX);
}

export function displayNameError(value: string): Message | undefined {
  return named(value, DISPLAY_NAME_MAX);
}

export function usernameError(value: string): Message | undefined {
  return /^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(value) && value.length <= USERNAME_MAX ? undefined : message("setup.usernameShape");
}

/** The email as the cloud keeps it (trimmed, lowercased), or `null` for one it would refuse. */
export function normalizeEmail(value: string): string | null {
  const email = value.trim().toLowerCase();
  const parts = email.split("@");
  const [local, domain] = parts;
  if (parts.length !== 2 || local === undefined || domain === undefined || local === "" || email.length > EMAIL_MAX) {
    return null;
  }
  const dotted = domain.includes(".") && !domain.startsWith(".") && !domain.endsWith(".") && !domain.includes("..");
  return dotted && !/[\s<>,;:"()[\]\\]/.test(email) ? email : null;
}

export function emailError(value: string): Message | undefined {
  return normalizeEmail(value) === null ? message("error.emailInvalid") : undefined;
}

/** A code as typed (`h8rw-2nce`, `H8RW 2NCE`): 8 characters once the case, spaces and the hyphen are set aside. */
export function codeError(value: string): Message | undefined {
  return value.replace(/[\s-]/g, "").length === 8 ? undefined : message("error.codeShape");
}

/** What a form field holds after trimming; a repository hint is free text the server keeps. */
export function hintError(value: string): Message | undefined {
  return length(value.trim()) > 300 ? message("error.hintLong") : undefined;
}
