import { Auth0Client } from "@auth0/nextjs-auth0/server";

/**
 * Server-only Auth0 client (Authorization Code + PKCE, state and nonce checks by the SDK).
 * Tokens live in an encrypted, HttpOnly, SameSite=Lax session cookie; the browser never
 * receives the access token. Returns null when any required setting is missing, so the UI
 * shows "sign-in not configured" instead of a false success.
 */
const required = [
  "AUTH0_DOMAIN",
  "AUTH0_CLIENT_ID",
  "AUTH0_CLIENT_SECRET",
  "AUTH0_SECRET",
  "APP_BASE_URL",
  "AUTH0_AUDIENCE",
] as const;

export const auth0Configured = required.every((k) => Boolean(process.env[k]?.trim()));

export const auth0: Auth0Client | null = auth0Configured
  ? new Auth0Client({
      authorizationParameters: {
        audience: process.env.AUTH0_AUDIENCE,
        scope: "openid profile email",
      },
      // The access token is only used server-side by the /api/loan proxy.
      enableAccessTokenEndpoint: false,
    })
  : null;
