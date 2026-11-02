# Frontend implementation: three isolated chat sessions

Status: **planned 2026-11-02; not implemented**. Baseline: the phase 3 MVP web app in
`frontend/` (one page, ~450 lines: catalogue, reserve/subscribe, my reservations, staff
desk, Auth0 server-side login and `/api/loan/*` proxy). See
[backend phase 3 handoff](../backend/phase-3-handoff.md) and [contracts](../backend/contracts.md).

Sources: CIS3039-N ICA spec (scenario, Stage 4 demo media, marking criteria), the
Blackboard starter container diagram supplied by the user, and the accepted stack.
**The starter diagram is stale** for the API (Python FastAPI), proxy (Nginx/Let's
Encrypt) and Notifications (reads PostgreSQL). The accepted design is Rust Loan API,
Container Apps managed HTTPS ingress (ADR 0003), and a Notifications service fed by
the Loan outbox through Service Bus that never touches PostgreSQL. Its frontend
responsibilities (catalogue, reserve, subscribe, staff actions, OIDC + PKCE sign-in
with Auth0) still apply.

| Phase | Result | Depends on |
| --- | --- | --- |
| [1: Structure and UX foundation](phase-1-structure.md) | Routed app shell, role-aware navigation, split components, typed client, test tooling | Current `frontend/` |
| [2: Complete role flows and tests](phase-2-flows-and-tests.md) | Polished student/staff journeys, 401/403/409 handling, component + E2E tests against a fake Loan API | Phase 1 handoff |
| [3: Cloud, observability and demo evidence](phase-3-cloud-and-evidence.md) | Web observability, CI gates, Test deployment, live Auth0 checks, demo media script, diagram refresh | Phase 2 handoff; live Auth0 tenant for live steps |

Run one phase per new chat, in order. Each phase file has a copyable starting prompt.

## Fixed constraints (every phase)

- Next.js App Router, React 19, `@auth0/nextjs-auth0` v4. Keep the backend-for-frontend
  pattern: access tokens stay in the encrypted HttpOnly session; the browser only calls
  `/api/loan/*`; `/auth/access-token` stays disabled; POSTs keep the Origin check.
- The proxy forwards only contract routes and only `modelId` in bodies. Never send
  email, role or subject from the browser. UI role checks are display-only; the Loan
  API enforces RBAC.
- Use only routes in `docs/backend/contracts.md`. A frontend need that requires a new
  API field or route is recorded as a backend follow-up, not invented in the proxy.
- No new paid services. Keep the web container stateless (no server-side session store).
- Accessibility: semantic HTML, labels, keyboard use, `role="alert"`/`status` notices.

## Rules for every session

1. Work in `/home/cabozzz/dev/resource.center`. Follow AGENTS.md: Serena activation and
   memories (`core`, `architecture/selected_baseline`, `project/decisions`,
   `frontend/plan`), and log the exact prompt in AI_LOG.md.
2. Read this index, the selected phase and earlier frontend handoffs. Inspect the
   current `frontend/` source and working tree; preserve unrelated work.
3. Keep changes scoped to the phase. If a prerequisite is missing, do independent work
   and name the blocker. Do not mark skipped checks passed.
4. Run `make frontend-typecheck frontend-test frontend-build frontend-audit` plus the
   phase's new checks; `make check` at the end of phase 3.
5. Write `docs/frontend/phase-N-handoff.md` (date, status, branch/commit, changes,
   commands and outcomes, local vs live evidence, open issues, next-phase verdict).
   Update `mem:frontend/plan` in place, finish AI_LOG.md, run `serena memories check`
   and `git diff --check`.

## Assessment coverage

- Implementation 25%: working frontend over two cooperating backend services, secure
  OIDC/JWT and RBAC visible (phases 1–2).
- Testing 15%: component tests with fakes, E2E against a fake Loan API, run in CI (2–3).
- DevOps 25%: frontend build/test/deploy in the pipeline, health/readiness, correlation
  IDs and logs from the web tier (3).
- Stage 4 media: availability, reserve/collect/return, 403s, notification evidence and
  CI/CD — phase 3 provides the scripted demo path.
