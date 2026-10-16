/**
 * Reads the configuration when the server starts, so a missing or wrong variable stops it with its name at once.
 * Next.js only logs an error thrown here and goes on serving `500` to every page, so the process exits itself: a
 * deployment sees a stopped container (and its restart loop), not a site that answers.
 */
export async function register(): Promise<void> {
  if (process.env.NEXT_RUNTIME !== "nodejs") {
    return;
  }
  const { config, ConfigError } = await import("@/lib/server/config");
  try {
    config();
  } catch (error) {
    if (!(error instanceof ConfigError)) {
      throw error;
    }
    console.error(`Orchester Cloud web cannot start: ${error.message}`);
    process.exit(1);
  }
}
