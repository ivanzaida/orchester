/** What forms and their server actions share; safe in the browser. */

/** The hidden field that carries the CSRF token (`lib/server/csrf.ts`). */
export const CSRF_FIELD = "csrf";

/** A form field's text; a file or a missing field reads as empty. */
export function field(form: FormData, name: string): string {
  const value = form.get(name);
  return typeof value === "string" ? value : "";
}
