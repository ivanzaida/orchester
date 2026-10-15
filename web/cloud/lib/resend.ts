/** The wait between confirmation emails the site asks for itself (the designer's 60 s, `y4FZY`). */
export const RESEND_COOLDOWN_MS = 60_000;

/** *0:42*: minutes and seconds left, rounded up so it never shows 0:00 while still waiting. */
export function formatWait(ms: number): string {
  const seconds = Math.max(0, Math.ceil(ms / 1000));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
}
