# Phase 2 — Reliable notification processing

Status: **planned**. One isolated implementation chat after phase 1. Follow `docs/backend/README.md`.

## Goal and entry context

Prevent overlapping active claims from causing avoidable duplicate sends; recover from failures while keeping honest at-least-once semantics.

Read ADR 0004, `docs/backend/phase-1-handoff.md` and `contracts.md`. Serena: `architecture/messaging`, `architecture/persistence`, `architecture/testing`, `architecture/service_boundaries`. Inspect Notifications application/domain/infrastructure, the publisher and shared contract. Verify phase 1 event identity/schema and outbox guarantees before changing delivery behaviour.

## Work

1. Define the job state/lease transition table in `docs/backend/notification-recovery.md`: claim owner/token, expiry, next retry time, attempt budget, terminal state and expected broker action for each outcome. Keep durable scheduling state in Cosmos; only temporary execution state lives in memory.
2. Implement conditional claims that reject a live competing lease. Support new, retry-due and expired jobs. Make renew/finish/failure writes require the current owner/token and ETag. Add compatible defaults or a controlled migration for existing job documents; do not replay all old deliveries after a schema change.
3. Bound email-call timeouts and renew both Service Bus lock and Cosmos lease when needed. Handle renewal failure and cancellation; stop starting sends after loss of ownership. Use a tested retry/defer strategy for active duplicates, avoiding hot loops and delivery-count exhaustion. Verify selected Service Bus Basic APIs/features before relying on them.
4. Retain provider-independent email ports and a deterministic fake sender. Verify Resend's supported idempotency key and retention before using it. Keep the stable key across retries/replay where supported; record what remains uncertain after a send succeeds but durable recording fails.
5. Define bounded retries/backoff, malformed/unsupported message handling, terminal failures, DLQ inspection/replay and graceful shutdown. Complete the broker message only after the durable outcome or a proven durable retry handoff. Recheck publisher lease recovery, broker acknowledgement and bounded batches.
6. Choose and record retention/replay windows together. Do not expire deduplication records before an allowed retry/replay can arrive. Redact PII in logs; use correlation IDs to link Loan, publisher, worker and delivery outcomes.
7. Preserve Loan database isolation and independent processing. Broker/email failures must not reverse completed Loan transactions; recovery must drain retained work.

## Completion checks

- Deterministic tests force two workers to compete while the first sender is paused: one active send; second claimant cannot take a live lease. Test stale owner completion rejection and expired-lease recovery.
- Test terminal duplicates, retry delay/limits, lost renewals, process crash before/after send, malformed/unsupported versions and replay. Explicitly document residual external-send duplicate risk.
- Real Cosmos tests prove conditional claim/renew/finalize conflicts across workers; fake-store tests alone are insufficient.
- Controlled integration checks prove broker outage retains outbox work; notification outage permits Loan commit; recovery drains backlog; all three event types and multiple subscribers reach the fake sender. Keep live email for explicitly authorized recipients in phase 3.
- Run focused Rust checks and add clear commands for real-service checks. Distinguish blocked external checks from passing local tests.

## Scope and handoff

Do not replace the queue/database stack or add cross-service callbacks. Save `docs/backend/phase-2-handoff.md`, the recovery document, tests, job schema compatibility rules, timeout/lease/retry settings and evidence. Phase 3 needs reliable fake delivery and the exact live-provider configuration contract. Phase 4 needs recovery/replay commands and observable signals.

## Starting prompt

```text
Implement phase 2 of the Resource Center backend plan in /home/cabozzz/dev/resource.center. Read AGENTS.md, activate this Serena project, then read backend/plan, docs/backend/README.md, docs/backend/phase-2-notifications.md, ADR 0004 and the phase-1 handoff/contracts. Verify prerequisites from files. Implement notification claim ownership, retry/recovery and concurrency tests without claiming exactly-once email. Preserve service isolation and existing delivery records. Complete the phase-2 handoff, update current memories and AI_LOG.md, and state any blocked integration checks. Do not start phase 3.
```
