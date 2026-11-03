# Frontend phase 3 handoff — cloud, observability and demo evidence

- Date: 2026-11-03
- Status: **implemented and locally verified; not deployed**. No deployment, live
  email, load test against Test or paid change was made (each needs explicit approval).
- Branch: `frontend/phase-3-cloud-evidence`, cut from `origin/qa` (`8669a8b`). PRs target `qa`.

## Changes

- **Observability** (`frontend/src/lib/observability.ts`, `src/app/api/loan/[...path]/route.ts`):
  one JSON log line per proxy request — `method`, `route` (template, IDs replaced by
  `{id}`, unknown paths logged as `unmatched`), `status`, `duration_ms`, `correlation_id`,
  `level` from status. No tokens, emails, bodies or raw IDs. Incoming `x-correlation-id`
  is forwarded only if it matches `[A-Za-z0-9._:-]{1,128}`; otherwise `web-<uuid>` is
  minted. Proxy-generated errors now also carry the header and `correlationId`.
- **Readiness** (`src/app/api/ready/route.ts`): 200 only when `APP_BASE_URL` is set,
  Auth0 is configured (or `ALLOW_UNCONFIGURED_AUTH=true` for approved local smoke)
  and the Loan API answers `/health/live` within 2.5 s; otherwise 503 with the failing
  part. `/api/health` stays the liveness/startup probe.
- **Terraform** (`infra/workloads/main.tf`): web readiness probe → `/api/ready`.
  Trade-off: if the Loan API is down the web replica is taken out of rotation instead
  of serving the degraded banner. Not applied.
- **Smoke** (`infra/scripts/smoke.py`, `infra/tests/test_smoke.py`): deployed smoke also
  requires web `/api/ready` with `loanApi: reachable` (skipped for the explicit
  unconfigured-auth local smoke).
- **CI/CD**: `ci.yml` frontend job now installs Chromium, runs Playwright E2E against
  the fake API and uploads `playwright-report` (HTML reporter when `CI` is set).
  `make check` now includes `frontend-e2e` (which installs Chromium), and `deploy.yml`
  verify installs Chromium system deps before `make check`. Web image build, Test
  release order and staging/production gates were already shared with the backend
  (backend phase 4) and are unchanged.
- **Configuration**: `infra/workloads` sets `AUTH0_DOMAIN`, `AUTH0_CLIENT_ID`,
  `AUTH0_AUDIENCE`, `APP_BASE_URL`, `LOAN_API_BASE_URL` as env, and
  `AUTH0_CLIENT_SECRET`/`AUTH0_SECRET` from web Key Vault (`auth0-client-secret`,
  `auth0-secret`) only when `handoff.web_vault_uri` is set. Names verified; values not.
- **Docs**: C4 container diagram in `docs/cloud/assessment-evidence.md` now shows
  managed ingress, Next.js + BFF proxy, the Auth0 PKCE flow, server-side token calls
  and web logs; portal row updated. New `docs/frontend/demo-script.md` maps each Stage 4
  media item to a route and action. `mem:frontend/plan` updated.

## Commands and outcomes (local)

| Command | Result |
| --- | --- |
| `make frontend-typecheck` | pass |
| `make frontend-test` | pass, 29 tests (3 new observability tests) |
| `make frontend-e2e` | pass, 11 tests (2 new: health/readiness, correlation forward/replace) |
| `python3 -m unittest discover -s infra/tests` | pass, 13 tests |
| `terraform fmt -check -recursive infra` | pass |
| `make check` | pass (exit 0): Rust fmt/clippy/tests, release tests, frontend typecheck/29 unit/build/audit/11 E2E, Terraform fmt + validate for test/staging/production |
| `actionlint` | not run — not installed locally |

## Live evidence (Test)

None in this phase. Blocked items and exact missing inputs:

| Check | Blocked on |
| --- | --- |
| Test deployment of the web image with run links | Approval to push to `qa` / dispatch Deploy; GitHub environment vars (`AZURE_*`, `ACR_*`, `WORKLOADS_TFVARS`, `WORKLOADS_BACKEND_HCL`) per release runbook |
| Student/staff sign-in, reserve → collect → return, student 403 | Auth0 tenant/app/API with roles claim, student + staff test users, `auth0-client-secret`/`auth0-secret` in web Key Vault and `web_vault_uri` in the handoff |
| Availability email | Backend live-email setup (Resend key, approved recipients) and explicit approval |
| Load/scale evidence | Deployed release plus approval to run `infra/scripts/load-check.py` against Test |

## Open issues

- `APP_BASE_URL` in `infra/workloads/main.tf` hard-codes the Test Container Apps
  environment domain suffix; staging/production need it derived from the platform
  handoff before promotion.
- `NEXT_PUBLIC_LOAN_API_URL` is still set on the web app; only a fallback in
  `loanApiBaseUrl()`; it could be removed once `LOAN_API_BASE_URL` is guaranteed.
- GitHub run of the new CI E2E job not yet observed.

## Verdict

Local scope complete. Remaining phase 3 items are live: approve a `qa` push/deploy,
then configure Auth0 and run the [demo script](demo-script.md).
