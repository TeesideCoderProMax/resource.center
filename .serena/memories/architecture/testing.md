# Acceptance evidence
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Pure domain/use-case tests with fake ports and deterministic clocks/IDs where relevant.
- Real PostgreSQL: concurrent reservations, allocation invariants, rollback, outbox atomicity.
- Publisher: broker outages, duplicate publication, restart recovery.
- Notifications: duplicate messages/concurrent consumers, crash windows, transient failure, exhausted retries, unsupported versions.
- Cosmos integration: chosen partitioning, conditional updates and concurrent job state transitions.
- Contract tests and full return-to-notification flow; prove loan operations survive Notifications/Resend outage and backlog resumes afterward.
- Verify separate deployment/configuration boundaries, least privilege, correlation logging and backlog/failure monitoring.
- No executable test suite exists yet; these are required design acceptance checks, not completed tests or confirmed rubric criteria.
