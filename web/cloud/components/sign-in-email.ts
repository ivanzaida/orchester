"use client";

/**
 * *Sign in on the web* after a confirmed email fills the address in on the sign-in page (`oFMIA`, the designer's
 * Account notes). It travels in this tab's session storage, never in a URL, where it would reach logs and history.
 */
const KEY = "orchester-sign-in-email";

export function rememberSignInEmail(email: string): void {
  sessionStorage.setItem(KEY, email);
}

/** The remembered address, once: it is removed as it is read. */
export function takeSignInEmail(): string | null {
  const email = sessionStorage.getItem(KEY);
  sessionStorage.removeItem(KEY);
  return email;
}
