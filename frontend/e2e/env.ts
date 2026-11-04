/** Shared E2E settings. All values are local test values; the Auth0 domain is never contacted. */
export const WEB_PORT = Number(process.env.E2E_WEB_PORT || 3100);
export const API_PORT = Number(process.env.E2E_API_PORT || 4011);
export const BASE_URL = `http://localhost:${WEB_PORT}`;
export const API_URL = `http://127.0.0.1:${API_PORT}`;
export const SESSION_SECRET = "e2e-only-session-secret-0123456789abcdef0123456789";

export const webEnv = {
  LOAN_API_BASE_URL: API_URL,
  APP_BASE_URL: BASE_URL,
  APP_ENV_NAME: "e2e",
  AUTH0_DOMAIN: "e2e.auth0.test",
  AUTH0_CLIENT_ID: "e2e-client",
  AUTH0_CLIENT_SECRET: "e2e-client-secret",
  AUTH0_SECRET: SESSION_SECRET,
  AUTH0_AUDIENCE: "https://loan-api.e2e.test",
  PORT: String(WEB_PORT),
};
