# Delivery semantics
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Loan publisher polls/claims committed PostgreSQL outbox rows; database does not automatically push. Mark publication only after broker acknowledgement.
- Initial return transaction also saves loan-selected notification requests. Later asynchronous fan-out needs explicit recovery/consistency design.
- At-least-once: broker acceptance before publication mark and email send before success recording both create duplicate windows. Do not promise exactly-once email; verify provider idempotency capabilities before reliance.
- Worker durably records/recognizes Cosmos job; delivers via Resend adapter; acknowledges message only after durable handling of outcome.
- Proposed contract: eventId, type=DeviceAvailabilityNotificationRequested, version=1, occurredAt, correlationId, recipient, deviceModel. Notification request, not delivery fact. Finalize required fields/version compatibility/validation.
- Message carries enough delivery data for no cross-service lookups; minimize PII, define retention/access/log redaction.
- Pending: stable IDs and partition-aware deduplication, concurrent claims, lock renewal, bounded retries with explicit owner per failure category, poison handling/replay, terminal failures.
- Email failure never rolls back committed loan operations. Broker outages retain recoverable outbox; define capacity/retention monitoring and recovery.
