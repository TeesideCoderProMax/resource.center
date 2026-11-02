# Frontend phase 3 — cloud, observability and demo evidence

Goal: ship the web app through the pipeline to Test, prove it live where inputs exist,
and prepare Stage 4 media.

## Scope

1. **Observability (web tier)**: structured JSON logs from the `/api/loan/*` proxy
   (method, route template, status, duration, correlation ID; never tokens/emails);
   generate or forward `x-correlation-id`; `/api/health` (liveness) and a readiness
   route that checks config and Loan API reachability; Container Apps probes use them.
2. **CI/CD**: frontend jobs in `.github/workflows/ci.yml` (typecheck, unit, build,
   audit, Playwright E2E with the fake API, upload report artifact); image build and
   Test deploy in `deploy.yml` following the backend phase 4 release order; existing
   staging/production gates apply to the web image too.
3. **Configuration per environment**: verify `AUTH0_DOMAIN`, `AUTH0_CLIENT_ID`,
   `AUTH0_AUDIENCE`, `APP_BASE_URL`, `LOAN_API_BASE_URL` and secrets
   `AUTH0_CLIENT_SECRET`, `AUTH0_SECRET` (names only) in `infra/workloads`.
4. **Live checks (needs Auth0 tenant and user approval)**: sign in as student and
   staff on Test; reserve → collect → return; student 403; subscribe and see the
   availability email (with backend live-email setup). Record results separately from
   local evidence.
5. **Scalability evidence**: a short load test against the web + API on Test (k6 or
   similar) or a scale-out screenshot; web stays stateless.
6. **Demo and docs**: `docs/frontend/demo-script.md` mapping each Stage 4 media item
   to a route and action; update the C4 container diagram so the web container shows
   Next.js + BFF proxy, Container Apps ingress (not Nginx), Rust Loan API and the
   Service Bus–fed Notifications service.

## Out of scope

New product features; production deployment without explicit user approval.

## Done when

- CI runs frontend checks and E2E; Test deployment of the web image is recorded with
  run links. Live checks pass or are listed as blocked with the exact missing input.
- `make check` passes; handoff and demo script written; memories updated.

## Starting prompt

```text
Implement frontend phase 3 from docs/frontend/phase-3-cloud-and-evidence.md in
/home/cabozzz/dev/resource.center. Read the frontend phase 1–2 handoffs and backend
phase 4 docs first. Follow AGENTS.md and docs/frontend/README.md. Ask before any
deployment, live email or paid change. Write the phase 3 handoff when done.
```
