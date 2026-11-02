# Phase 4 handoff — 2026-11-02

Status: **local implementation complete; live delivery and submission partial**.
Base: latest fetched `origin/main`, `7419d1a`. Branch:
`codex/backend-phase-4-delivery`. Clean worktree created at
`/home/cabozzz/.codex/worktrees/backend-phase-4/resource.center` before the task log.
Serena is active there. Implementation commits: `8687a95` (seed safety) and
`b877d88` (release/check tooling). The later commit request authorizes branch
publication; no deployment or email send occurred.

## Result

- `infra/scripts/release.py` and `.github/workflows/deploy.yml`: one release path
  for all environments. Reviewed isolated config/state, exact image overrides,
  stop old API/web/worker revisions, wait for zero replicas, prepare only the
  migration job, wait for the migration/seed transaction, then apply apps and
  smoke. First release works after platform and images exist. Failure leaves
  the release stopped; there is no unsafe automatic old-image restart.
- `promotion.py`: promote only the digests in a successful main-push Test release
  artifact. Check repository/run/SHA/version/smoke identity and actual independent
  reviewer policy. Import to target ACR and verify unchanged digests. No typed
  confirmation or arbitrary digest input acts as an approval gate.
- CI now runs real PostgreSQL contention/rollback/seed checks, proxy security tests,
  release policy/failure tests and runtime image smoke/load checks. Untrusted PRs
  receive no deployment credentials. Deploy verification publishes its check log;
  a successful release publishes `release.json`. Optional manual Cosmos checks use
  a separate authorized identity and send no mail.
- `demo_seed.sql`: insert units only for newly inserted demo models. Reruns preserve
  staff edits, removed units, stock counts and reservations. Conflicting unit/serial
  IDs abort the transaction, so new model counts cannot silently become wrong.
  All three assessment environment examples now enable repeatable seeds.
- Smoke checks use explicit target URLs and both release versions. They check
  readiness/database, catalogue, correlation and auth rejection. Web health uses
  the same Auth0 configuration test as the web client. Auth0 must be configured
  for deployed smoke; the isolated local image check explicitly allows no tenant.
- Terraform: web release version env, publisher timeout raised from 60 to 960
  seconds for the configured bounded batch, and invalid KQL `nonempty` calls fixed.
  No platform or workload change was applied.
- Runbook and current C4/Stage 2 status, report outline, evidence/media checklist,
  root README, clean ZIP tool, Serena status and AI log updated. Source packaging
  requires final identity/report/media inputs and never submits to Blackboard.

## Checks and evidence

| Check | Result and scope |
| --- | --- |
| `make check` | Passed: Rust fmt/clippy, 27 default Rust tests, frontend typecheck/3 proxy tests/build/audit (0 vulnerabilities), release tests and Terraform fmt/validation for all three environments. Later focused checks cover final edits. |
| `make backend-test-postgres` | Passed: 3 suites on disposable PostgreSQL 17 with verified TLS. Existing migration, five-way contention, transaction rollback and return tests; new seed preservation and collision rollback tests. |
| `make release-test` | Passed: 13 local Python tests. Migration failure/cancel/timeout, drain timeout (including inactive revisions on retry), wrong state, seed policy, release order/digest overrides, promotion policy/provenance, smoke identity/auth, bounded HTTP probe, ZIP exclusions/layout. Cloud calls are mocked. ZIP/media test inputs are synthetic; ffprobe is mocked there. |
| `make docker-build` | Both runtime images built locally; final backend seed rebuild also passed. No image pushed. |
| `make image-test` | Actual images with isolated PostgreSQL/TLS; migrate and seed twice, API/web smoke and 40 catalogue GETs with concurrency 5. No Auth0, broker, Cosmos or email in this check. Final rebuilt image check passed: 40 requests, zero errors, p50 18.76 ms, p95 27.92 ms over 0.167 seconds. This is local single-instance evidence. |
| `actionlint 1.7.7`, shell syntax, `git diff --check` | Passed, including the final workflow and whitespace checks. |
| `serena memories check` | Passed. |
| Repository/gate access | School GitHub connector confirms private repository and admin metadata. It exposes no environment-settings tool. Local `gh` environment query returns 404. Actual rules and plan support remain unverified; no controls changed. |

The full suite ran before the last small seed/test/timeout edits. Final focused
checks rerun affected Rust/PostgreSQL, Python, image, Terraform and workflow paths;
no passed local result substitutes for GitHub execution. Current results:
`phase-4-checks.json`. Raw temporary check logs are under `/tmp/phase4-*.log` on this
machine; they are not durable cloud evidence and are not part of the submission.

## Environment status

| Environment | Phase 4 outcome | Run/release IDs |
| --- | --- | --- |
| Local | Code, policy, SQL, frontend, image and packaging-tool checks | Base `7419d1a`; implementation commits above; local image tags `rc-backend:ci`, `rc-web:ci` |
| Test | Prior 2026-11-01 deployment evidence retained. No new rollout. Auth0/live email and real Cosmos acceptance remain open. | No phase 4 GitHub run, deployment or migration execution ID |
| Staging | Reviewed templates and shared release path only; no provisioning | None |
| Production | Isolated assessment template and shared path only; no provisioning | None |

No external runtime settings or current replica counts were inspected. Older
`docs/cloud/test-*-checks.json` records remain dated evidence. The former cloud
rollback predates migration 0002; it cannot prove current-schema image rollback.

## Remaining work and recovery

1. Configure the actual per-environment workflow variables, OIDC trust and private
   state handoffs. The new workflow deliberately refuses to use example values.
   Configure supported independent reviewer rules and protect main; record an
   unapproved attempt blocked and a reviewed promotion using the tested digests.
2. Set up Auth0 tenant/claims, web secret **env bindings**, and approved test users.
   Prove real login, roles/403, browser actions and live emails to authorized
   recipients. The phase 3 Terraform did not wire web secret env references.
3. Resolve Cosmos data access (prior test got 403). Run real conflict, broker
   outage, backlog drain, replica/restart and scale checks. Run corrected KQL and
   capture supported alert routing; Azure Students blocked the older test.
4. With commit/push/deploy authorization, capture commit → checks → images → Test
   migration → smoke. Capture a failed check blocking release. No such authority
   or workflow run was assumed in this session.
5. Review schema compatibility and prove image rollback/forward recovery on the
   current schema. Migration failure leaves apps stopped. On smoke failure, new
   apps can already be active; inspect logs and stop writes before recovery.
   Use [release runbook](release-runbook.md); never restart pre-0002 writers or
   reverse SQL by deleting migration records.
6. Confirm tutor Stage 2 approval, final student identity, report and media. The
   packager is ready but final ZIP status is partial. Use the
   [submission checklist](../assessment/submission-checklist.md). No media or
   personal reflection was invented.

No further implementation phase is assumed. Live acceptance and final submission
are the remaining work; their absence does not undo the local results above.

Final review: local document links and Serena references checked. The first final Python run hit the sandbox loopback restriction; the approved rerun passed all 13 tests. No live checks were added.
