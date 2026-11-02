import { defineConfig } from "@playwright/test";
import { API_PORT, BASE_URL, WEB_PORT, webEnv } from "./e2e/env";

/** E2E against the built app (`next build` first) and the fake Loan API. */
export default defineConfig({
  testDir: "e2e",
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [["list"]],
  use: { baseURL: BASE_URL, trace: "retain-on-failure" },
  webServer: [
    {
      command: "node fake/loan-api.mjs",
      env: { FAKE_LOAN_API_PORT: String(API_PORT) },
      url: `http://127.0.0.1:${API_PORT}/health/live`,
      reuseExistingServer: false,
    },
    {
      command: `npx next start -p ${WEB_PORT}`,
      env: webEnv,
      url: `${BASE_URL}/api/health`,
      reuseExistingServer: false,
    },
  ],
});
