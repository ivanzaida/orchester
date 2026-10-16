import { defineConfig } from "@playwright/test";

/**
 * The end-to-end run against the real binaries (`tests/e2e`): a team server, Orchester Cloud on a throwaway Docker
 * PostgreSQL, both web apps' production builds, in the installed Edge. Build first, from the repository root:
 *   cargo build -p orchester-server -p orchester-cloud
 *   npm --prefix web run build --workspace cloud && npm --prefix web run build --workspace server
 * then `npm run test:e2e` in web/server. Each file starts and stops its own services, one file at a time.
 */
export default defineConfig({
  testDir: "tests/e2e",
  testMatch: "*.e2e.ts",
  fullyParallel: false,
  workers: 1,
  timeout: 120_000,
  reporter: [["list"]],
  use: {
    channel: "msedge",
    viewport: { width: 1440, height: 900 },
    ignoreHTTPSErrors: true,
  },
});
