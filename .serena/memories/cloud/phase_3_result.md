# Delivery and operations evidence
Stored evidence date 2026-11-01. Source review separates implemented files from runtime proof.
- Sources: docs/cloud/phase-3-handoff.md, test-phase3-checks.json, runbook.md; .github/workflows/{ci,deploy}.yml; Makefile; infra/*/monitoring.tf.
- CI/deploy YAML implements PR/push checks, Test deploy on main, immutable digests, migration wait and smoke checks. Promotion uses workflow_dispatch, environment names and typed confirmation. These do not prove GitHub required-reviewer protection or a successful commit-driven deploy; verify actual runs/settings.
- Test rollout currently precedes migration execution. Promotion branch lacks the Test branch's migration wait and smoke checks. Review these release gaps before claiming safe, complete promotion.
- Staging/Production templates exist with separate state and Cosmos serverless; no deployment proof. Demo seeding is disabled there despite brief's per-deployment seed instruction.
- Stored checks record local make check, image rollback/restore, PostgreSQL 5-way contention, publisher lease recovery, three fake-email notification outcomes and a 40-request catalogue burst. They do not prove all resilience cases or live Auth0/email.
- Monitoring code and stored checks show structured logs, correlation, probes, KQL queries and six alerts. Synthetic alert-routing test was blocked by Azure Students policy; actual delivery is not verified.
- Keep precise execution IDs/results in evidence files. Remaining checks: `mem:architecture/testing`; assessment actions: `mem:assessment/requirements`.
