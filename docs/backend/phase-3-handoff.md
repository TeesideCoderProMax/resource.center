# Phase 3 handoff — 2026-11-02

Status: **local implementation complete; live acceptance blocked**.
Branch: `codex/backend-phase-3-secure-flows`, based on `main` at `3591ccf` (phase 2 merged).
No commit, push or deployment in this session.

## Result

- **Loan API auth** (`backend/crates/loan-service/src/auth.rs`, moved out of
  `infrastructure.rs`): RS256 only; `exp`, `iss`, `aud`, `sub` required (30 s leeway).
  Roles come only from `AUTH0_ROLES_CLAIM`; the old `roles`/`permissions` fallbacks
  are removed. The recipient email comes only from `AUTH0_EMAIL_CLAIM`; the invented
  `<sub>@auth0.user` fallback is removed, so a token without email gets 401.
  The issuer falls back to `https://<AUTH0_DOMAIN>/`, which fixes a gap: Terraform
  set `AUTH0_DOMAIN` but the API read only `AUTH0_ISSUER`.
- **JWKS**: 10 min cache. An unknown `kid` forces a refresh, at most once per 30 s.
  A failed refresh keeps the cached keys. No usable keys gives 503 `AuthUnavailable`,
  never success. Auth errors return fixed messages; details go to logs only.
- **Web app** (`frontend/`): `@auth0/nextjs-auth0` 4.33 (React bumped to 19.2 for its
  peer range). Authorization Code + PKCE with SDK state/nonce checks; `/auth/login`,
  `/auth/logout`, `/auth/callback` via `src/proxy.ts`. Screens: public catalogue
  (brand/model, category, availability), reserve with due date shown, subscribe when
  out of stock, my reservations, and a staff desk to find, collect and return.
  Errors such as 409 last-unit conflicts show a clear message, not success. The UI
  reloads state from the API after every action.
- **Token storage**: tokens stay in the SDK's encrypted HttpOnly session cookie
  (SameSite=Lax). The browser calls `/api/loan/*`, a server proxy that adds the
  access token. `/auth/access-token` is off, so page scripts (and so XSS) cannot read
  the token. CSRF: POSTs need an `Origin` equal to `APP_BASE_URL`, plus SameSite=Lax.
  The proxy forwards only contract routes and only `modelId` in bodies, so a client
  cannot send its own email, role or subject. Roles shown in the UI are for display;
  the Loan API enforces them.
- **Terraform** (`infra/workloads/main.tf`, not applied): the web container now gets
  `AUTH0_DOMAIN`, `AUTH0_CLIENT_ID`, `AUTH0_AUDIENCE`, `APP_BASE_URL` instead of
  unused `NEXT_PUBLIC_AUTH0_*` names.

Reservation queries were already done in phase 1 (`GET /api/v1/reservations`, owner
filter for students, all rows for staff; collect/return staff-only). No API route,
schema, migration or event contract changed. Contract notes: [contracts.md](contracts.md).

## Validation (2026-11-02, local)

- `make backend-fmt backend-lint backend-test`: passed. Four new auth tests: missing
  and malformed header, unconfigured, bad signature, wrong issuer, wrong audience,
  expired, missing email, role claim trust, JWKS cache, unknown-kid refresh limit,
  JWKS outage (fail closed) and stale-key use. Existing route tests still prove
  student 403 on collect, cross-user read rejection and staff actions.
- `make frontend-typecheck frontend-test frontend-build frontend-audit`: passed;
  3 proxy tests (route allowlist, cross-origin POST rejected, body injection removed);
  0 vulnerabilities.
- `terraform fmt -check` on tracked dirs and `terraform validate` (workloads): passed.
  `make infra-fmt` fails only on the git-ignored local `infra/environments/test/workloads.tfvars`.
- Local smoke on the standalone build without Auth0: page shows "Sign-in not
  configured"; cross-origin POST → 403; same-origin POST → 503 `AuthNotConfigured`;
  unknown route → 404. Client bundle has no secret names or access token.
- Not run: `make backend-test-postgres` (no SQL change), browser automation, any live test.

## Blocked live checks

No Auth0 tenant is set in any environment (`auth0_*` tfvars are empty). No email
recipients are authorized. So these remain **not done**:

1. Create an Auth0 API (identifier = `AUTH0_AUDIENCE`) and a Regular Web Application
   (callback `<APP_BASE_URL>/auth/callback`, logout `<APP_BASE_URL>`). Add a
   post-login Action that sets `https://resource-center.campus/roles` (from Auth0
   RBAC roles `student`/`staff`) and `https://resource-center.campus/email` on the
   access token. Create one student and one staff test user.
2. Store web secrets as Container App secrets: `AUTH0_CLIENT_SECRET`, `AUTH0_SECRET`
   (names only; values never in git). Set `auth0_domain`, `auth0_audience`,
   `auth0_client_id` in tfvars and apply through the reviewed Test process.
3. Live proof: sign-in, real 403 for a student on collect, all three email types
   with two subscribers, via `EMAIL_DELIVERY_MODE=resend`, `RESEND_FROM_ADDRESS`,
   `RESEND_AUTHORIZED_RECIPIENTS` and Key Vault `resend-api-key`. Phase 2 Cosmos
   403 and broker checks also remain open.

## Next phase

Phase 4 is **ready for local work** (release order, CI, smoke). Live end-to-end
evidence is **not ready** until the steps above are done. Tutor approval is unknown.
