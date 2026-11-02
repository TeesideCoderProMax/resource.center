# Frontend phase 2 — complete role flows and tests

Goal: make every scenario journey clear and robust, and prove it with automated tests.

## Scope

1. **Student flows**: reserve shows the assigned due date (`dueAt`) and next steps;
   subscribe shows confirmation and "you will be emailed when available"; reservation
   list with status badges and overdue highlight (`dueAt` < now and not returned).
2. **Staff flows**: filter by status (RESERVED/COLLECTED), search, collect/return with
   confirm step, timestamps shown after each action, list refresh from the API.
3. **Error and edge handling**: 401 → re-sign-in prompt; 403 `ForbiddenRole` → explicit
   403 message (demo evidence); 409 last-unit race → suggest subscribe; 409
   `InvalidTransition` → refresh row; 503 auth/API unavailable → degraded banner while
   the public catalogue still renders (graceful degradation). Show the correlation ID
   on errors for support.
4. **Double-submit protection**: disable buttons while pending; no optimistic count
   changes — always reload from the API (server stays authoritative).
5. **Tests**:
   - Component tests (Vitest + RTL) for each flow and error mapping.
   - A fake Loan API (MSW or a small Node server implementing the contract routes and
     error envelopes) used by tests and by `npm run dev:fake`.
   - Playwright E2E against the built app + fake API with a stubbed session for
     student and staff: reserve, last-unit 409, subscribe, staff collect/return,
     student 403 on `/staff` and on a forced collect call, cross-origin POST 403.
   - Add `make frontend-e2e`.

## Out of scope

Live Auth0, deployment, CI wiring (phase 3), new backend routes (record as follow-ups).

## Done when

- All journeys above pass in E2E locally; component and proxy tests pass.
- Existing make targets pass; handoff written with any backend follow-ups listed.

## Starting prompt

```text
Implement frontend phase 2 from docs/frontend/phase-2-flows-and-tests.md in
/home/cabozzz/dev/resource.center. Read docs/frontend/phase-1-handoff.md first.
Follow AGENTS.md and docs/frontend/README.md. Write the phase 2 handoff when done.
```
