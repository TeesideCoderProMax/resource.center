# Phase 4 — Safe delivery and assessment evidence

Status: **implemented and checked locally; live acceptance and submission partial**. Results: [phase 4 handoff](phase-4-handoff.md). The requirements below remain the scope checklist, not proof that all live checks ran.

## Goal and entry context

Prove that the corrected system builds, tests and deploys safely, and prepare accurate assessment evidence. Do not replace actual CI, live security or email proof with local configuration screenshots.

Read ADR 0004 and all three handoffs. Serena: `architecture/testing`, `assessment/cloud_module_criteria`, `cloud/phase_3_result`, `suggested_commands`, `task_completion`. Inspect current workflows, Terraform environment settings, migration/seed commands, smoke scripts and stored evidence. Check repository/environment access read-only before assuming gates exist.

## Work

1. Run meaningful unit/integration suites in CI, including real PostgreSQL contention/rollback and notification concurrency checks with documented fakes and optional real-service jobs. Keep untrusted PRs away from secrets. Publish readable test outcomes and release identity.
2. Correct release ordering: build immutable images once; prepare compatible migrations in the target private network; wait for success; seed safe deterministic assessment data; then activate the new application revision and run functional/health smoke checks. Keep a failed migration from activating the new revision. Account for existing Terraform ownership and first-release job image availability.
3. Share the release path with Staging/Production promotion, including migration wait, seed policy and smoke checks. Promote the tested image digests; keep environment config/state/identities isolated. Demo seeds must be repeatable and must not reset real data. Reconcile the brief's each-deployment seed requirement explicitly for each assessment environment.
4. Inspect and configure real approval controls supported by the repository plan. Demonstrate unauthorized/unapproved promotion is blocked and approved promotion uses the verified images. A typed confirmation is not proof of independent reviewer protection; explain the actual control. If a target environment is not deployed, record that limit rather than provisioning paid resources without scope.
5. With the session's commit/push/deploy authorization, capture a real commit → test → build → Test deployment run. Show a failing check blocks release. If that authorization/access is absent, finish the reviewable workflow changes and record runtime proof pending.
6. Verify structured logs/correlation across services, health/readiness, job duration/backlog/error metrics and alerts. Capture permitted alert evidence; if Azure Students blocks synthetic routing tests, preserve the limitation and use a supported test only within authorization.
7. Run bounded resilience/scaling checks: concurrent reservations; notification outage with successful Loan commits; backlog recovery; replica/restart recovery; load or autoscaling with workload, latency/errors, replica counts and duration. Reuse valid phase evidence and rerun only where changed code or unproved behaviour requires it.
8. Prove image rollback/forward recovery against compatible schema; document why schema rollback is different. Update runbook, current C4/Stage 2 diagrams and the requirement→code→test→demo evidence map.
9. Prepare submission materials: capture supported screenshots and 20–30-second clips, total MP4 H.264 duration under five minutes; clean Source packaging; Documentation; root readme with supplied name/user ID. Keep secrets, Terraform state, caches and build/dependency folders out of the ZIP. Supply a 1,500–2,000-word report outline and evidence/tradeoff checklist for the student's own decisions; review a supplied draft without inventing personal reflection, tutor sign-off or unseen media.

## Completion checks

- `make check`, image builds and the added meaningful integration suites pass; tests are visible in real CI when access permits.
- Migration failure blocks new-revision activation; seed rerun is safe; approved release passes smoke tests; gated promotion and rollback have actual evidence or an explicit unverified status.
- Logs, metrics, health/readiness, alert and scaling/recovery results map to rubric requirements. Live/fake modes and remaining limitations are explicit.
- Validate submission folder structure, source cleanliness, media format/duration and evidence links. If student identity, report, media or tutor approval is missing, leave packaging/submission status partial. Do not submit to Blackboard.
- Run `serena memories check` and `git diff --check`; remove obsolete active phase instructions and retain concise current status/evidence links.

## Handoff

Save `docs/backend/phase-4-handoff.md` with final per-environment status, release/run IDs, validation commands/results, evidence index, remaining blockers and recovery instructions. Update the plan index to show actual phase outcomes, not a blanket completed claim. No further phase is assumed.
