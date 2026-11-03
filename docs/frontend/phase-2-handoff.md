# Frontend phase 2 handoff — role flows and tests

- Date: 2026-11-02
- Status: **implemented and locally verified** against a fake Loan API with stubbed Auth0
  sessions. Not deployed; no live Auth0 or real Loan API used.
- Branch: `frontend/phase-2-flows`, cut from `qa` (`dcf2c73`). PRs target `qa`.

## Changes

- **Error handling** (`lib/api-client.ts` `errorHint()`, `components/Notice.tsx`
  `errorNotice()`): 401 → "Sign in again" link with `returnTo`; 403 `ForbiddenRole` →
  explicit "403 Forbidden."; 409 `NoAvailableUnits` → subscribe offered at once;
  409 `InvalidTransition` → message plus list reload; 502/503 → "Service degraded"
  banner. Every error keeps the correlation ref.
- **Student** (`DeviceCard`): reserve shows unit, `dueAt` and desk next step with a link
  to My reservations; subscribe confirms "You will be emailed when … is available".
  Buttons disabled and `aria-busy` while pending; repeated clicks ignored. No optimistic
  counts: the parent reloads from the API after each attempt.
- **Reservations** (`ReservationTable`): status colour classes, `isOverdue()` (due date
  passed and not returned) with row highlight and "Overdue" badge.
- **Staff** (`StaffDesk`): status filter (open/reserved/collected/returned/all), search,
  Refresh button, two-step Mark → Confirm/Cancel, success notice with
  `collectedAt`/`returnedAt` from the response, reload after every action.
- **Catalogue**: server fetch failure renders a degraded banner; the public page still renders.
- **Fake Loan API** (`frontend/fake/loan-api.mjs`): in-memory contract routes and error
  envelopes, correlation header, `/__reset`, `/__seed`, `/__events`. It decodes bearer
  payloads without verifying signatures — test-only, never deployed (not copied by the
  standalone image output). `npm run dev:fake` runs it with `next dev` (signed-out only,
  since no Auth0 tenant).
- **Tests**: Vitest component tests for DeviceCard, StaffDesk, ReservationTable,
  Notice/Catalogue (fetch stubbed via `src/test/fake-fetch.ts`); RTL `cleanup` added to
  `vitest.setup.ts`. Playwright (`playwright.config.ts`, `e2e/`) runs the built app via
  `next start` plus the fake API. Sessions are real encrypted SDK cookies made with
  `@auth0/nextjs-auth0/testing` `generateSessionCookie`, so production proxy/session code
  is unchanged and has no test switch. New `make frontend-e2e` (build + Playwright),
  added to `make check`.

## Commands and outcomes (local)

| Command | Result |
| --- | --- |
| `make frontend-typecheck` | pass |
| `make frontend-test` | pass, 8 files / 26 tests |
| `make frontend-build` | pass |
| `make frontend-audit` | 0 vulnerabilities |
| `make frontend-e2e` | pass, 9 tests: signed-out catalogue, reserve + due date, last-unit 409 → subscribe, staff collect/return with confirm, overdue highlight, student 403 on `/staff` and forced collect, cross-origin POST 403, signed-out 401, body identity injection ignored |
| `npm run dev:fake` + curl `/` | catalogue rendered from the fake API |
| `git diff --check` | clean |

## Not verified

- Live Auth0 sign-in and the real Rust Loan API (phase 3). The fake API mirrors
  `docs/backend/contracts.md` but is not a contract test against the Rust service.
- 503 degraded path is covered by component tests only, not E2E.
- Visual light/dark/mobile review not performed. `make check` and `make docker-build` not run.

## Backend follow-ups

- None required. Optional: a single-model `GET /catalogue/{id}` route and a student
  view of their own subscriptions would simplify the detail page and confirmation UX.

## Verdict

Phase 3 can start: wire `make frontend-e2e` into CI (needs Playwright Chromium in the
runner), then run the live Auth0 checks.
