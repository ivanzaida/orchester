import { defineConfig } from "@playwright/test";

/**
 * Page tests: the built admin against the fake team server and the fake cloud (`tests/fake-api/server.ts`), all on
 * loopback, in the Edge that is installed on this computer (Playwright downloads no browser). `npm run test:pages`.
 */
const WEB_PORT = 4200;
const API_PORT = 4210;
const CLOUD_PORT = 4220;

export const TEST_ENV = {
  ORCHESTER_ADMIN_SERVER_URL: `http://127.0.0.1:${API_PORT}`,
  ORCHESTER_ADMIN_CLOUD_URL: `http://127.0.0.1:${CLOUD_PORT}`,
  // A fixed test secret of 32 bytes; production reads its own from the environment.
  ORCHESTER_ADMIN_SECRET: Buffer.alloc(32, 5).toString("base64"),
  NEXT_TELEMETRY_DISABLED: "1",
};

export default defineConfig({
  testDir: "tests/pages",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: `http://localhost:${WEB_PORT}`,
    channel: "msedge",
    viewport: { width: 1440, height: 900 },
  },
  webServer: [
    {
      command: `node tests/fake-api/server.ts ${API_PORT} ${CLOUD_PORT}`,
      url: `http://127.0.0.1:${API_PORT}/__fake/requests`,
      reuseExistingServer: false,
    },
    {
      command: `npx next start -H 127.0.0.1 -p ${WEB_PORT}`,
      url: `http://localhost:${WEB_PORT}/admin/https`,
      env: TEST_ENV,
      reuseExistingServer: false,
    },
  ],
});
