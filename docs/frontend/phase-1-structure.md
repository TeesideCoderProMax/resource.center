# Frontend phase 1 — structure and UX foundation

Goal: turn the single-page MVP into a maintainable, routed app without changing
behaviour or the security model.

## Scope

1. **Routes** (App Router):
   - `/` public catalogue (brand/model, category, total/available counts; counts may
     need sign-in only if the spec wording is enforced — keep public as today unless
     the user decides otherwise).
   - `/devices/[id]` model detail with reserve/subscribe actions and loan duration.
   - `/reservations` signed-in user's loans (status, reserved, due, collected, returned).
   - `/staff` staff desk (search by reservation/student/model, collect, return).
   - Server-side guards: unauthenticated → sign-in prompt; non-staff on `/staff` → a
     clear "403 — staff only" page (display only; API still enforces).
2. **App shell**: header with sign-in/out, user email and role badge, role-aware nav,
   footer with environment name. `loading.tsx`, `error.tsx`, `not-found.tsx`.
3. **Code split**: move `ui.tsx` into `src/components/` (Catalogue, DeviceCard,
   ReservationTable, StaffDesk, Notice) and a typed client `src/lib/api-client.ts`
   (one `call()` with error mapping, correlation ID read from responses).
4. **Styling**: keep plain CSS (CSS modules or `globals.css` tokens); no UI framework
   unless justified in the handoff. Light/dark, mobile width.
5. **Test tooling**: add Vitest + React Testing Library + jsdom; migrate the existing
   `node --test` proxy tests or keep both under `npm test`. Add first tests for the
   error-message mapping and role guard helpers.

## Out of scope

New API routes, E2E browser tests, deployment, Auth0 tenant setup.

## Done when

- All four routes render; existing flows still work end to end locally against the
  Loan API (or show "Sign-in not configured" without Auth0).
- `make frontend-typecheck frontend-test frontend-build frontend-audit` pass; client
  bundle still contains no secrets or access token.
- `docs/frontend/phase-1-handoff.md` written.

## Starting prompt

```text
Implement frontend phase 1 from docs/frontend/phase-1-structure.md in
/home/cabozzz/dev/resource.center. Follow AGENTS.md and docs/frontend/README.md.
Write the phase 1 handoff when done.
```
