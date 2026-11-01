# Persistence
Source: backend migrations/adapters, infra/platform/data.tf and ADR 0003; reviewed 2026-11-01.
- Loan alone owns PostgreSQL. Reservation allocation uses a transaction and row locks; collection/return transitions and notification outbox rows commit together. Notifications has no PostgreSQL credentials.
- Current due_at is nullable at reservation and set at collection; the cloud brief requires a return date on reservation. See `mem:assessment/requirements`.
- Notifications alone owns Cosmos jobs: /eventId partition, id = stable single-recipient eventId, conditional ETag updates and duplicate-create handling. These do not alone prove exclusive sending under concurrent duplicate requests.
- Test Cosmos uses free-tier shared manual 400 RU/s; PostgreSQL is provisioned managed compute. Neither is scale-to-zero. Retention/TTL and replay windows still need explicit policy.
- Queue and email lie outside PostgreSQL transactions. No distributed ACID or exactly-once email claim.
- Stored integration evidence: `mem:cloud/phase_3_result`; test gaps: `mem:architecture/testing`.
