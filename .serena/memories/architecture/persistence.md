# Persistence
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- PostgreSQL private to Loan; Cosmos DB for NoSQL private to Notifications. No shared credentials or database access between capabilities.
- Reservation allocation and outbox write share one PostgreSQL transaction; return transition and resulting requests also commit atomically.
- Cosmos/email are outside this local transaction; no distributed ACID claim.
- Pending PostgreSQL concurrency/locking strategy must enforce atomic allocation under contention.
- Pending Cosmos capacity mode, partition key, conditional job updates and deduplication scope. IDs must not be assumed unique across arbitrary partitions. Assess serverless for intermittent workload.
