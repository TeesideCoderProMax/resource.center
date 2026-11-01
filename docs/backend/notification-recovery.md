# Notification recovery

Implemented locally on 2026-11-01. Real Cosmos validation is blocked by HTTP 403.
See [phase 2 handoff](phase-2-handoff.md) for evidence limits.

## State and broker action

Cosmos owns timing and attempt state. Each claim gets a random `claimToken`, which
identifies that execution, and `leaseUntil`. ETags fence writes. A worker checks
its token and unexpired lease before renewal or outcome writes; every replacement
uses the ETag it obtained at claim or renewal. A stale write gets HTTP 412.

| Stored state | Condition | Action | Broker action |
| --- | --- | --- | --- |
| Missing | Valid v1 request | Create claimed job; attempt 1 | Keep lock |
| Claimed | Lease still live | Do not send or change owner | Hold lock; poll at most every 10s |
| Claimed | Lease expired, budget remains | Claim with new token and ETag; increment attempt | Keep lock |
| RetryableFailure | Retry time not reached | Do not send | Hold lock; wait up to 10s per poll |
| RetryableFailure | Retry due, budget remains | Claim with new token and ETag; increment attempt | Keep lock |
| Claimed or RetryableFailure | Eligible but budget spent | Record TerminalFailure with ETag | Complete after write |
| Claimed | Send succeeds | Record Delivered with current ETag | Complete after write |
| Claimed | Transient failure or timeout | Record retry time, or terminal when budget spent | Wait on the same locked delivery, or complete terminal |
| Claimed | Permanent error or disallowed recipient | Record TerminalFailure | Complete after write |
| Delivered or TerminalFailure | Duplicate/replay | Do not send | Complete |
| Any | ETag conflict or lost Cosmos renewal | Cancel pending send; read state again | Keep lock and wait |
| Any | Store outage | Retain claim and request; wait 5s | Keep renewing lock |
| Any | Broker renewal fails | Cancel processing; do not settle | Let lock and lease expire |
| Any | Shutdown | Cancel processing and renewal; retain work | Let lock and lease expire |
| No valid contract | Malformed JSON or unsupported version/type | Do not send or create a job | Abandon; broker DLQs after its delivery limit |

A crash consumes an attempt once a claim exists, even if no email starts. This
bounds repeated crashes. The last uncertain attempt can end as TerminalFailure;
that status does not prove the provider did not accept the email.

## Bounds and compatibility

- Cosmos lease: 60s; renewal every 10s while sending. Email operation: 20s total.
- Broker lock: Test config is 60s. Renew every 10s, with a 10s renewal deadline.
- REST clients: 30s request timeout. Token acquisition has its existing 15s bound.
- `MAX_DELIVERY_ATTEMPTS`: default 5, clamped to 1–10.
- Retry delay: `min(5 * 2^attempt, 300)` seconds. Defaults give 10, 20, 40, 80s.
- Busy jobs and retry waits keep the current broker delivery locked. They do not
  abandon in a loop or consume delivery count. This holds one worker slot; use
  multiple worker replicas for throughput. No scheduled-message or session
  feature is required.
- Old documents may omit the three new fields. Terminal jobs remain terminal.
  A legacy claim becomes eligible at `updatedAt + 60s`. Drain old worker revisions
  before rollout: their old reclaim rule does not honor leases. No bulk replay,
  delete, or document migration is needed.
- Keep event IDs and payloads unchanged on retry/replay. Current v1 event identity
  and recipient selection remain Loan-owned. Notifications never queries Loan.
- Publisher batches are clamped to 1–100. Each publication has a 35s total timeout.
  The batch lease is at least `35 * batch_size + 60` seconds. PostgreSQL keeps
  unacknowledged rows PENDING; only broker acknowledgement permits PUBLISHED.
  A crash or lost acknowledgement can still cause a repeat publication.

## Retention and replay

Test queue TTL is 14 days. Allowed routine replay is within 14 days of occurrence.
Cosmos TTL stays disabled (`-1`); retain terminal deduplication records indefinitely
until a separate enforced replay cutoff and cleanup policy exist. Do not enable
TTL as a space-saving change. Terminal writes erase the raw recipient address.
Provider response bodies do not enter errors or logs. Use event and correlation
IDs for tracing; normal recipient logs use the existing redacted address.

Resend uses the stable event ID in `Idempotency-Key`. Its documented cache is
24 hours, not 14 days. Keep the sender address and email template unchanged for
in-flight retries. A successful external send followed by a failed Cosmos write
can send again after lease recovery, especially after 24 hours. Cancelling a
request cannot undo an email already accepted by the provider. No exactly-once
email guarantee exists.

Verified provider/API references on 2026-11-01:

- [Resend idempotency keys](https://resend.com/changelog/idempotency-keys): 24h retention.
- [Resend key format](https://resend.com/blog/engineering-idempotency-keys): 1–256 characters.
- [Service Bus REST renewal](https://learn.microsoft.com/en-us/rest/api/servicebus/renew-lock-for-a-message): POST to the locked message URI.
- [Service Bus tiers](https://azure.microsoft.com/en-us/pricing/details/service-bus/): Basic queues and lock renewal operations. This design uses ordinary queue peek-lock, renewal, completion and unlock.

## Inspection and replay procedure

1. In Azure Portal, open the Test Service Bus queue, then Service Bus Explorer.
   Select the dead-letter subqueue and **Peek**. Record message ID, delivery count,
   reason and correlation ID. Treat message bodies as private recipient data.
2. Point-read Cosmos `jobs` using `id = eventId` and partition key `eventId`.
   Inspect status, attempts, lease expiry and retry time. Do not delete the job.
3. Fix the cause first. A terminal job will remain suppressed on replay. Review
   provider evidence for uncertain sends; do not reset terminal jobs blindly.
4. For a valid nonterminal job within 14 days, use Explorer to resend the exact
   original v1 body to the original queue. Preserve event ID and correlation ID.
   Wait for the send acknowledgement before removing the DLQ copy. If interrupted,
   resend with the same ID; retained Cosmos state provides deduplication.
5. For malformed data, keep the original evidence. A corrected request needs a
   reviewed source fix. Do not invent recipients or bypass the version contract.
6. Check the Cosmos terminal outcome and correlated worker logs. A fake outcome
   proves processing only; it does not prove received email.

Normal backlog recovery needs no replay command: restart the publisher and worker.
Use `loan-service publish` for one bounded batch and `notifications-worker drain-once`
for an explicit queue drain, with each process's own existing environment/identity.
Never give Notifications the publisher's PostgreSQL credentials.

## Checks

`make backend-test` runs deterministic fake tests. `make backend-test-postgres`
runs real disposable PostgreSQL tests. `make backend-test-cosmos` requires
`COSMOS_ENDPOINT`, optional `COSMOS_DATABASE`/`COSMOS_CONTAINER`, and a managed
identity with Cosmos data access (or the existing short-lived
`AZURE_STATIC_TOKEN_OVERRIDE` for the Cosmos audience). It creates one unique
synthetic document, tests concurrent create/renew/finish conflicts, then deletes
only that document. It sends no email. Do not print the token.

Real broker outage, worker outage/backlog drain, deployed lock renewal and
multi-recipient end-to-end checks still need a controlled Test run. Local fake
and PostgreSQL results do not replace those checks.
