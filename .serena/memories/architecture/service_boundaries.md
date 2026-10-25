# Ownership
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Loan owns catalogue/devices/availability, reservations/two-day loans/collections/returns, subscription/waitlist eligibility, recipient selection, authoritative PostgreSQL state, transactional outbox and publisher progress.
- Notifications owns jobs/templates/message preparation, delivery attempts/status, retry schedules/failed deliveries, processed event IDs, required delivery recipient data, Cosmos storage and Resend integration.
- Notifications neither queries PostgreSQL nor calls Loan API to reconstruct ordinary requests. Loan API neither reads nor writes notification Cosmos records.
- Exchange explicit versioned messages, never private schemas/persistence models. Eligibility remains Loan logic; Notifications formats and delivers.
- Independent Notifications is accepted and supersedes earlier shared-database alternatives. Preserve history, do not silently revert.
- Avoid frontend business orchestration, entity-only services, unnecessary synchronous chains. Distinguish C4, deployment and Cosmos containers.
