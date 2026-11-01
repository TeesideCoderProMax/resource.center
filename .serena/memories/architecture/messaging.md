# Delivery semantics
Source: backend application/adapters and rc-shared contract; reviewed 2026-11-01.
- Loan emits versioned NotificationRequestV1 for collection confirmation, return confirmation and availability. Include recipient/content and correlation identity so Notifications needs no Loan lookup. Loan owns eligibility and recipient selection.
- Publisher claims committed outbox rows with recoverable leases; mark published only after broker acknowledgement. Failures retain retryable work. Current Test broker is Service Bus Basic.
- At-least-once delivery: acknowledgement/write and email-send/outcome-record gaps can duplicate work. Stable event IDs and Cosmos ETags reduce duplicates but do not prove exactly-once sending.
- Notifications uses timed claim tokens and ETags; renews Cosmos/broker leases; holds broker locks through durable retry waits. Local tests cover competing workers, cancellation and crash gaps. Real Cosmos returned 403; deployed recovery checks remain open. Rules and replay/retention: docs/backend/notification-recovery.md.
- Keep bounded retries, poison/dead-letter handling, lock expiry/recovery, retention/replay rules and PII redaction explicit. Loan commits must survive Notifications/Resend outages.
- Current deployed evidence uses fake email. Live recipient delivery and full outage/backlog recovery are separate checks: `mem:architecture/testing`.
