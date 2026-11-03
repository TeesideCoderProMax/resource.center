import { generateSessionCookie } from "@auth0/nextjs-auth0/testing";
import type { BrowserContext } from "@playwright/test";
import { BASE_URL, SESSION_SECRET } from "./env";

export type Persona = "student" | "student2" | "staff" | "norole";

const PEOPLE: Record<Persona, { sub: string; email: string; roles: string[] }> = {
  student: { sub: "auth0|student-1", email: "student1@campus.test", roles: ["student"] },
  student2: { sub: "auth0|student-2", email: "student2@campus.test", roles: ["student"] },
  staff: { sub: "auth0|staff-1", email: "staff1@campus.test", roles: ["staff"] },
  norole: { sub: "auth0|norole-1", email: "norole@campus.test", roles: [] },
};

/** Unsigned token: the fake Loan API decodes it; the web tier only reads roles for display. */
function fakeAccessToken(p: (typeof PEOPLE)[Persona]): string {
  const b64 = (o: object) => Buffer.from(JSON.stringify(o)).toString("base64url");
  return `${b64({ alg: "none", typ: "JWT" })}.${b64({
    sub: p.sub,
    "https://resource-center.campus/email": p.email,
    "https://resource-center.campus/roles": p.roles,
  })}.e2e`;
}

/**
 * Stubbed sign-in: an encrypted Auth0 SDK session cookie made with the SDK's own testing
 * helper, so the production session/proxy code path runs unchanged.
 */
export async function signIn(context: BrowserContext, persona: Persona) {
  const p = PEOPLE[persona];
  const now = Math.floor(Date.now() / 1000);
  const value = await generateSessionCookie(
    {
      user: { sub: p.sub, email: p.email, name: p.email },
      tokenSet: { accessToken: fakeAccessToken(p), expiresAt: now + 3600 },
      internal: { sid: `e2e-${persona}`, createdAt: now },
    },
    { secret: SESSION_SECRET },
  );
  await context.addCookies([{ name: "__session", value, url: BASE_URL, httpOnly: true, sameSite: "Lax" }]);
  return p;
}
