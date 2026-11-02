# Phase 2 handoff — 2026-11-01

Status: **local implementation complete; real-service acceptance partial/blocked**.
Branch: `codex/backend-phase-2-notifications`, based on updated `main` at `14e897d`.
The worktree was clean before the task log. Backend commit: `4125ae1`.
Published through the School GitHub account in [PR #2](https://github.com/TeesideCoderProMax/resource.center/pull/2). No deployment occurred.

## Result

Notifications now uses timed claim tokens, ETag fencing, lease renewal, durable
retry times, bounded attempts and email deadlines. Busy duplicates wait on their
current broker delivery instead of using up delivery count. Store outages retain
the broker lock. Broker renewal loss and shutdown cancel processing and leave
work for expiry recovery. Terminal duplicates never send again.

Old terminal documents stay terminal; missing lease fields have compatible
read defaults. Drain old workers before rollout. No SQL migration or v1 contract
change was made. Loan still owns recipients, business state and PostgreSQL;
Notifications owns Cosmos and delivery. No callback or cross-database access was
added. The publisher now bounds batch size and total send time and gives the
whole sequential batch enough lease time.

[Recovery rules](notification-recovery.md) contain the transition table, exact
timeouts/backoff, old-document rules, retention/replay policy, Basic REST API
references, Resend's 24h key window and the DLQ procedure. Cosmos TTL stays off.
Email acceptance followed by a crash or failed outcome write can still duplicate
an external send. The test suite explicitly demonstrates this gap.

## Files

- `backend/crates/notifications-worker/src/{domain,application,infrastructure,main}.rs`:
  state, processing, REST bounds/redaction and cancellation.
- `backend/crates/notifications-worker/src/cosmos_tests.rs`, its Cargo manifest
  and root Makefile: opt-in real Cosmos conflict checks and test clock support.
- `backend/crates/loan-service/src/{application,infrastructure}.rs`: bounded
  publisher batches and calls; omit broker error response bodies.
- This handoff, recovery guide, plan index/status, Serena memories and AI_LOG.md.

## Validation

- `make backend-fmt`, `make backend-lint`, `make backend-test`: passed. The local
  suite has 23 passing tests and two opt-in tests ignored by default.
- Twelve Notifications tests cover paused competing senders, expired recovery,
  stale completion, lost Cosmos renewal, timeout/attempt limits, terminal/legacy
  duplicates, delayed retry, cancellation, invalid contracts, retained fake
  backlog across all three event types/four recipients, and the uncertain-send
  crash gap. The retained fake backlog is not a real broker outage test.
- `make backend-test-postgres`: passed with disposable Docker PostgreSQL and TLS.
  It checked fresh/existing migrations, contention, Loan rules, rollback,
  concurrent return and multi-recipient outbox contents. The existing publisher
  fake test proves failed publication does not mark PUBLISHED, then recovery does.
- `make backend-test-cosmos`: attempted against the recorded Test endpoint using
  the existing Azure login and a short-lived Cosmos token. Real Cosmos returned
  **403 Forbidden on create**. No claim/renew/finalize success is claimed. The
  opt-in test is ready for the Notifications identity with Cosmos data access.
- `git diff --check` and `serena memories check`: passed.

## Limits and next phase

Test, Staging and Production were not changed. No live email was sent. The
existing Test deployment remains the previous release; local tests do not prove
that deployment uses this code. No RBAC grants were added to bypass the 403.

Still required before phase 2 service acceptance:

1. Run `make backend-test-cosmos` with Cosmos data access and prove real conflicts.
2. Roll out through the normal reviewed Test process, draining old workers first.
3. Test real broker outage/retained outbox, worker outage while Loan commits,
   backlog drain, broker renewal failure, and all three types/multiple recipients
   reaching fake delivery. Inspect/replay a controlled DLQ item.
4. Check shutdown during an actual provider call. Cancellation cannot withdraw
   a send already accepted by the provider.

Phase 3 may start independent local Auth0/UI work from these contracts, but the
real phase 2 reliability gate is **not ready**. Phase 3 owns approved live email
and browser proof. Provider config remains `EMAIL_DELIVERY_MODE` (`fake` default,
`resend` live), `RESEND_FROM_ADDRESS`, `RESEND_AUTHORIZED_RECIPIENTS`, and either
`RESEND_API_KEY` or `KEYVAULT_URI`/`RESEND_SECRET_NAME` (default `resend-api-key`).
Synthetic requests always use fake delivery. Use stable sender/template settings
for outstanding retries. Tutor approval status remains unknown.
