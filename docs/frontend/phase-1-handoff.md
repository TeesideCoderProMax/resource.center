# Frontend phase 1 handoff — structure and UX foundation

- Date: 2026-11-02
- Status: **implemented and locally verified** (build, unit/component tests, no-Auth0 smoke). Not committed at time of writing; not deployed.
- Branch: `codex/backend-phase-3-secure-flows` (base commit `bd21388`).

## Changes

- **Routes** (`frontend/src/app/`): `/` public catalogue; `/devices/[id]` detail with
  reserve/subscribe and loan duration (reads `GET /catalogue`, since the contract has no
  single-model route); `/reservations` (own loans, or all for staff); `/staff` desk.
  Server-side display guards via `lib/roles.ts#access()` render `AccessNotice`:
  sign-in prompt (with `returnTo`), "Sign-in not configured", "No role assigned", and
  "403 — staff only". The Loan API still enforces RBAC.
- **Shell**: `layout.tsx` + `components/AppHeader.tsx` (role-aware nav, email, role
  badge, sign-in/out), footer with `APP_ENV_NAME` (new, in `.env.example`, default
  `local`), skip link, `loading.tsx`, `error.tsx`, `not-found.tsx`.
- **Code split**: `ui.tsx` removed; `components/` has `Catalogue`, `DeviceCard`,
  `ReservationTable` (+ `useReservations`, `MyReservations`), `StaffDesk`, `Notice`,
  `AccessNotice`, `AppHeader`. `lib/api-client.ts` has one `call()`, error-code
  mapping (`errorMessage`), `ApiClientError` carrying status/code/correlation ID
  (from the `x-correlation-id` response header), and `describe()` which appends the
  ref. Server helpers `getViewer()`/`fetchCatalogue()` in `lib/viewer.ts`.
  The cross-component `reservations-changed` window event was dropped: catalogue and
  reservations now live on separate routes and refetch on navigation.
- **Styling**: plain CSS tokens in `globals.css`, dark default plus
  `prefers-color-scheme: light`, focus outlines, horizontally scrollable tables for
  mobile width. No UI framework.
- **Tests**: Vitest 3 + React Testing Library + jsdom (`vitest.config.mts`,
  `vitest.setup.ts`); `npm test` = `vitest run`. The `node --test` proxy tests were
  migrated (`bff.test.ts`). New: `api-client.test.ts`, `roles.test.ts`,
  `components/AccessNotice.test.tsx`.
- Security model unchanged: proxy, Origin check, `modelId`-only bodies and disabled
  `/auth/access-token` untouched.

## Commands and outcomes (local)

| Command | Result |
| --- | --- |
| `make frontend-typecheck` | pass |
| `make frontend-test` | pass, 4 files / 11 tests |
| `make frontend-build` | pass; routes `/`, `/devices/[id]`, `/reservations`, `/staff` |
| `make frontend-audit` | 0 vulnerabilities |
| `next start` without Auth0/Loan API env, curl each route | `/`, `/devices/x`, `/reservations`, `/staff` 200 with "Sign-in not configured" / "LOAN_API_BASE_URL is not configured"; unknown path 404 |
| `grep -rE 'AUTH0_CLIENT_SECRET\|AUTH0_SECRET\|accessToken\|tokenSet' .next/static` | no matches |

## Not verified

- End-to-end flows against a running Loan API with a live Auth0 session (signed-in
  reserve/subscribe/collect/return, the 403 page with a real student token). No Auth0
  tenant or Loan API was running in this session.
- Visual check in a browser (light/dark, mobile width) was not performed.
- `npm` warned that `esbuild` (Vitest dev dependency) has an unapproved install script;
  tests ran regardless. Confirm the Docker `npm ci` build is unaffected (`make docker-build` not run).

## Open issues / phase 2 notes

- Detail page uses `notFound()` for unknown IDs; with Loan API down it shows the error instead.
- Layout and page both call `getViewer()` (two session reads per request); acceptable, could be cached with React `cache()`.
- 401 handling still shows a message only; phase 2 owns redirect-to-sign-in and 409 UX.

## Verdict

Phase 2 can start: structure, typed client and test tooling are in place. Run the
signed-in local flow check first.
