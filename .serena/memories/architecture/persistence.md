# Persistence
- Loan alone owns PostgreSQL. Application-owned transaction scopes lock model → reservation/unit → subscriptions → outbox; commit state and notification requests together. No remote calls in a SQL transaction.
- Phase 1 schema/date/subscription rules and rollout limits: docs/backend/contracts.md. Migration 0002 preserves valid recorded dates, backfills null dates from reservation time, validates due dates and enforces only ACTIVE subscription uniqueness. It requires the matching API; old writers must drain before migration.
- Local real PostgreSQL migration/contention/partial-write rollback tests passed: docs/backend/phase-1-handoff.md. Not deployed by this phase.
- Notifications alone owns Cosmos jobs: /eventId partition, stable single-recipient event ID and conditional ETag writes. Exclusive concurrent sending is not yet proved; phase 2 owns leases/recovery.
- Queue/email lie outside PostgreSQL transactions. No distributed ACID or exactly-once email. Retention/replay remains open.
- Deployed infrastructure/evidence: `mem:cloud/plan`; test limits: `mem:architecture/testing`.
