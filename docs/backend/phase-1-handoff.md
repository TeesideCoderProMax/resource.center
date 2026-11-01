# Backend phase 1 handoff

Date: 2026-11-01. **Status: local implementation and checks complete. Tutor approval status unknown; deployment pending.** Base: `main`, commit `92462ae`. Changes are prepared on `codex/backend-phase-1-loan-core` for a PR to `main`. The follow-up request authorizes commit/push and the PR; deployment remains outside this work. Phase 2 has not started.

## Result

Loan now runs explicit reserve, subscribe, collect and return use cases through application-owned transaction scopes. Domain rules govern availability and `RESERVED → COLLECTED → RETURNED`. PostgreSQL implements storage and model-first locking. Errors roll back state and outbox together. Fake storage uses these same use cases.

Reservation assigns and returns a UTC due date from one clock value plus 48 hours. Collection records its time and keeps that due date. Early/repeated returns and repeated collection fail without extra events. Active duplicate subscriptions retain their ID/time; later subscription cycles keep past rows. Return consumes all active subscribers and creates one stable event per distinct email recipient, plus borrower confirmation.

`GET /api/v1/reservations` supplies student-owned rows or all owners for staff, bounded to 200 newest rows. Existing write routes remain unchanged. The v1 notification envelope remains compatible with the current worker.

## Changed files

- `backend/crates/loan-service/src/{domain,application,infrastructure,presentation,main}.rs`: rules, ports/use cases, PostgreSQL adapter, read route, due dates, migration runner, and tests. Composition still injects PostgreSQL; the blanket application implementation supplies the use-case interface.
- `backend/crates/loan-service/migrations/0002_reservation_due_dates.sql`: additive upgrade; migration 0001 and seed source remain unchanged.
- `backend/crates/loan-service/src/{test_support,postgres_tests}.rs`, `tests/fixtures/{test-only-rsa.pem,jwks.json}`: shared transactional fake, real database checks and public test-only JWT material. These fixtures are not live credentials; test constructors compile only under cfg(test).
- `backend/scripts/test-postgres.sh`, root `Makefile`: disposable PostgreSQL command with TLS, bounded startup wait and cleanup.
- `docs/backend/{contracts,stage-2-plan,phase-1-handoff}.md`: API/event/schema contract, annotated diagram and this result. Phase 1 plan/index and ADR 0004 now point to actual results.
- `docs/cloud/assessment-evidence.md`: current route/date claims and C4/deployment diagrams; old cloud test files remain untouched.
- `AGENTS.md`, affected `.serena/memories/` and `AI_LOG.md`: current phase status, next work, evidence and exact prompts. Existing memory upkeep rules in `.serena/project.yml` remain in force.

## Validation run in this chat

| Command / check | Outcome |
| --- | --- |
| `make backend-fmt` | Passed |
| `make backend-lint` | Passed; warnings denied |
| `make backend-test` | 13 passed: 9 Loan, 2 Notifications, 2 shared contract. The separate database test is ignored in this command. |
| `make backend-test-postgres` | Passed: 1 real PostgreSQL suite covering the cases below. Ran against a new local PostgreSQL 17 Docker container with verified TLS; container and certificates removed on exit. |
| `serena memories check` | Passed; no reference issues. |
| `git diff --check` and document/memory reference checks | Passed; links, fences and active memory targets resolve. |

Real database cases: fresh migration and repeated runner; five competing reservations with one winner; due date persistence/preservation; role and ownership checks; duplicate active subscription; collection failure at outbox insertion rolls back reservation/unit changes; return failure rolls back count/state/subscriptions; two concurrent returns with one winner; all subscriber events validated as v1; a second subscription/notification cycle; existing-date preservation, null-date backfill, invalid-date rejection before changes, NOT NULL enforcement and retained original rows.

Local tests also cover fixed-clock late collection, invalid/repeated transitions, distinct-unit allocation/reuse, stable event IDs, fake transaction rollback and owner/staff reads. HTTP tests use signed RS256 test tokens and cached test JWKS; they check route names, response dates, duplicate subscription IDs and 400/401/403/404/409 results. They do not call live Auth0 or send email. Existing worker tests still pass; no worker lease changes occurred.

An initial lint run found a test-module placement issue; it was fixed before the passing checks. Full `make check`, frontend builds, Terraform checks, CI execution and cloud checks were not needed or run for this backend-only change.

## Migration and rollout limits

See [contracts](contracts.md) for the inspection SQL and release steps. Migration 0002 preserves valid non-null dates and fills only null dates from reservation time. It aborts on invalid dates, with the transaction unchanged. Old schema subscription uniqueness becomes ACTIVE-only uniqueness; past rows remain intact.

**Old API writers are incompatible with migration 0002.** Drain/pause old write traffic, apply the migration, then activate the matching image and verify it. Do not roll back to the old image without a reviewed schema recovery plan. The existing pipeline does not yet enforce this order; phase 4 must address it before rollout. No live data inspection, schema upgrade or reset occurred in this phase.

## Deployment and external inputs

- Test: prior Azure deployment evidence exists; phase 1 changes are local only. Stored tests use fake email and show missing Auth0 config. This chat did not recheck live settings.
- Staging/Production: configuration templates exist; no new deployment or proof of live environments.
- Local PostgreSQL uses disposable test credentials and a temporary certificate, not cloud secrets. Live config names remain `AUTH0_ISSUER`, `AUTH0_AUDIENCE`, `AUTH0_ROLES_CLAIM`, PostgreSQL/managed identity settings, `EMAIL_DELIVERY_MODE` and the existing Key Vault `resend-api-key` secret.
- [Stage 2 annotated diagram](stage-2-plan.md) is ready for review. Tutor approval status for MVP/fakes/build order is unknown. The due-date interpretation is a design choice to discuss, not a separate approval explicitly named in the brief.
- Browser flows, live Auth0/JWKS refresh and email delivery, worker lease races, CI gates and submission media remain later-phase work.
- Read API has a 200-row bound and no paging. Axum extractor errors retain their default format. No cancellation/expiry/renewal workflow or exactly-once email guarantee is added.

## Next phase

**Phase 2 is ready for local implementation.** Read [contracts](contracts.md), [phase 2 plan](phase-2-notifications.md), and the current Notifications code. Stable event IDs, unchanged v1 payloads and tested Loan transaction/outbox rules are available. Phase 2 must add owned leases, retry/recovery and concurrent delivery tests. It must not infer tutor approval or live delivery from phase 1.

Phase 3 can use the documented owner/staff read route and reservation-time `dueAt`. Phase 4 must enforce the migration/write-traffic constraints before deployment.

## Final validation record

Serena memory checks, Markdown link/fence checks, active memory reference checks, shell syntax and `git diff --check` passed. Mermaid source was reviewed but not rendered in this chat. No cloud changes or live checks ran.
