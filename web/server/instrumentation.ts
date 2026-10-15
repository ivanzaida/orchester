/** Reads the configuration when the server starts, so a missing or wrong variable stops it with its name at once. */
export async function register(): Promise<void> {
  if (process.env.NEXT_RUNTIME === "nodejs") {
    const { config } = await import("@/lib/server/config");
    config();
  }
}
