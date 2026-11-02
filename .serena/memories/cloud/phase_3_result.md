# Delivery and operations evidence
- Stored cloud evidence dated 2026-11-01: docs/cloud/phase-3-handoff.md and test-phase3-checks.json. It records older Test rollback, contention, publisher recovery, fake delivery and a bounded catalogue burst, not current-schema or live Auth0/email proof.
- Current release implementation/results: docs/backend/phase-4-handoff.md. Shared migration-first maintenance release, artifact-based promotion and assessment seed policy supersede old workflow/typed-confirmation instructions. Runbook: docs/backend/release-runbook.md.
- Only Test has deployment evidence. Staging/Production remain templates. Actual GitHub gate settings, commit-driven deployment and current-schema rollback remain unverified; no phase 4 cloud changes.
- JSON logs, correlation, probes, saved KQL and six metric alert definitions exist. Phase 4 fixes invalid KQL syntax locally; query execution remains unverified. Azure Students blocked the stored synthetic alert-routing test; actual delivery remains unproved.
- Live recovery/scale checks: `mem:architecture/testing`; assessment inputs: `mem:assessment/requirements`.
