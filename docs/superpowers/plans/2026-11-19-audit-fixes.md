# QA Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the five findings from the 2026-11-19 ponytail audit of `qa` so one transient failure or one bad row cannot stop notifications, and Postgres connections are reused.

**Architecture:** Each fix stays in the layer and service that owns the code today: worker loop (Notifications application), outbox batch (Loan application), outbox claim and connection reuse (Loan Postgres adapter), router auth test (Loan presentation tests). No port signatures change.

**Tech Stack:** Rust, Tokio, tokio-postgres, Axum, `cargo test`; real-DB checks via `make backend-test-postgres`.

**Spec:** Audit findings 1–5 (AI_LOG entries 126–127, 2026-11-19) and the constraints in `AGENTS.md`.

## Global Constraints

- No new dependencies (finding 4 uses a small in-adapter idle list, not `deadpool`).
- Delivery stays at-least-once; duplicates are removed by the Notifications Cosmos job record. Never claim exactly-once.
- Loan owns PostgreSQL and the outbox; Notifications never touches PostgreSQL.
- `drain-once` mode still exits non-zero on any receive or complete error (CI drain job relies on it).
- Publisher job still exits non-zero when any row failed (`main.rs` checks `failed_count > 0`).
- At most `max_connections` (`PG_MAX_CONNECTIONS`, default 5) open Postgres connections per replica.

## Review Focus

1. A pooled connection closed by Azure (idle timeout, failover) must not be handed out — pinned by `pool_discards_closed_connection` (Task 4).
2. A request that errors mid-transaction must not leave the reused connection inside an open transaction — pinned by `pool_reuses_connection_after_rolled_back_transaction` (Task 4).
3. A row that failed to parse must not be claimed again on the next run — pinned by `unreadable_outbox_row_is_failed_and_others_publish` (Task 3, real DB).
4. After a failed complete in long-running mode the next message is still processed — pinned by `complete_failure_does_not_stop_long_running_loop` (Task 1).
5. After a `mark_published` error the remaining rows are still sent — pinned by `mark_published_error_does_not_abort_batch` (Task 2).

---

### Task 1: Worker survives a failed complete

**Files:**
- Modify: `backend/crates/notifications-worker/src/application.rs` (`run_consumer_loop`, `MessageDisposition::Complete` arm; tests module `FakeConsumer`)

**Interfaces:** none changed. `run_consumer_loop` keeps its signature.

- [ ] **Step 1: Write the failing test.** Add `complete_calls: AtomicUsize` to `FakeConsumer`, incremented at the start of `complete_message`.

```rust
#[tokio::test]
async fn complete_failure_does_not_stop_long_running_loop() {
    // inbox: [locked(valid_body("e1"), "lock-1")], fail_complete = true, drain_once = false
    // shutdown closure: ready once consumer.complete_calls >= 1, else pending
    let processed = run_consumer_loop(&consumer, &processor, false, shutdown).await;
    assert_eq!(processed, Ok(0));
    assert_eq!(consumer.complete_calls.load(Ordering::SeqCst), 1);
}
```
Keep `complete_failure_propagates` (drain-once) unchanged.

- [ ] **Step 2:** `cargo test -p notifications-worker complete_failure` → new test FAILS (`Err("complete failed")`), old one passes.
- [ ] **Step 3:** In the `Complete` arm: on `Err(err)`, return `Err(err)` when `drain_once`; otherwise `error!(event_id, error, "failed to complete message; broker will redeliver and Cosmos dedup prevents a second send")` and `continue`.
- [ ] **Step 4:** `cargo test -p notifications-worker` → all pass.
- [ ] **Step 5: Commit** `fix(worker): keep consuming after a failed message complete`

### Task 2: Publisher batch continues after a `mark_published` error

**Files:**
- Modify: `backend/crates/loan-service/src/application.rs` (`run_outbox_publish_batch` `Ok(())` arm; tests `FakeOutbox`)

**Interfaces:** none changed. `PublishBatchSummary` fields unchanged.

- [ ] **Step 1: Write the failing test.** Add `fail_mark_published: Option<String>` to `FakeOutbox`; `mark_published` returns `Err(ApplicationError::Persistence("db down".into()))` for that event id.

```rust
#[tokio::test]
async fn mark_published_error_does_not_abort_batch() {
    // items e1, e2; broker succeeds; fail_mark_published = Some("e1")
    let summary = run_outbox_publish_batch(&outbox, &broker, 10, 60).await.unwrap();
    assert_eq!(summary.claimed_count, 2);
    assert_eq!(summary.published_count, 1);
    assert_eq!(summary.failed_count, 1);
    assert_eq!(*outbox.published.lock().unwrap(), vec!["e2".to_string()]);
}
```
- [ ] **Step 2:** `cargo test -p loan-service mark_published_error` → FAIL (`unwrap` on `Err`).
- [ ] **Step 3:** Replace `store.mark_published(..).await?` with a match: on `Err`, `failed_count += 1`, `tracing::warn!(event_id, error, "published but not marked; row will be republished after lease expiry")`, do **not** call `record_publish_failure` (the row was sent; leaving the lease avoids an immediate duplicate).
- [ ] **Step 4:** `cargo test -p loan-service` → all pass.
- [ ] **Step 5: Commit** `fix(loan): finish the outbox batch when marking a row published fails`

### Task 3: Unreadable outbox row is failed, not blocking

**Files:**
- Modify: `backend/crates/loan-service/src/infrastructure.rs:704-760` (`claim_pending_batch`)
- Test: `backend/crates/loan-service/src/infrastructure.rs` (new `#[cfg(test)]` unit test) and `backend/crates/loan-service/src/postgres_tests.rs`

**Interfaces:**
- Produces (private, same file): `fn split_claimed_rows(rows: Vec<(String, String, i32, serde_json::Value)>) -> (Vec<ClaimedOutboxItem>, Vec<(String, String)>)` — parsed items, and `(event_id, error)` for rows whose payload fails `serde_json::from_value::<NotificationRequestV1>`.

- [ ] **Step 1: Write the failing tests.**

```rust
#[test]
fn split_claimed_rows_separates_unreadable_payloads() {
    // row a: serde_json::to_value(valid NotificationRequestV1); row b: json!({"bad": true})
    let (items, bad) = split_claimed_rows(rows);
    assert_eq!(items.len(), 1);
    assert_eq!(bad[0].0, "b");
    assert!(bad[0].1.starts_with("invalid outbox json"));
}
```
In `postgres_tests.rs`, `#[ignore]`-style like its neighbours: `unreadable_outbox_row_is_failed_and_others_publish` — insert one valid PENDING row and one with `payload = '{"bad":true}'`, call `claim_pending_batch(10, 60)`, assert 1 item returned, and the bad row has `status = 'FAILED'`, `claimed_until IS NULL`, `last_error LIKE 'invalid outbox json%'`; a second claim returns 0 items.

- [ ] **Step 2:** `cargo test -p loan-service split_claimed` → FAIL (not defined).
- [ ] **Step 3:** Implement `split_claimed_rows`; in `claim_pending_batch`, before `tx.commit()`, for each bad row run `UPDATE loan.outbox_events SET status = 'FAILED', claimed_until = NULL, last_error = $2 WHERE event_id = $1` and `tracing::error!(event_id, "outbox row payload unreadable; marked FAILED")`. `FAILED` is already allowed by the `0001` schema `CHECK`.
- [ ] **Step 4:** `cargo test -p loan-service` → pass; `make backend-test-postgres` → pass (if Docker is unavailable, record that in the ledger and say so in the final message).
- [ ] **Step 5: Commit** `fix(loan): mark unreadable outbox rows FAILED instead of blocking the publisher`

### Task 4: Reuse Postgres connections

**Files:**
- Modify: `backend/crates/loan-service/src/infrastructure.rs:40-108` (`PostgresAdapter`, `connect_client`)
- Test: `backend/crates/loan-service/src/postgres_tests.rs`

**Interfaces:**
- `PostgresAdapter` gains `idle: Arc<std::sync::Mutex<Vec<Client>>>`.
- Produces: `pub struct PooledClient { client: Option<Client>, idle: Arc<Mutex<Vec<Client>>>, max_idle: usize }` with `Deref<Target = Client>` and `DerefMut`; `Drop` pushes the client back when `!client.is_closed()` and `idle.len() < max_idle`.
- `connect_client(&self) -> Result<(PooledClient, OwnedSemaphorePermit), ApplicationError>` — same tuple shape, so the 23 existing `let (client, _permit)` / `let (mut client, _permit)` call sites compile unchanged.
- Order inside `connect_client`: acquire permit → pop idle clients, discarding any with `is_closed()` → if none, run today's connect code (token, TLS, spawn connection task).
- Add `// shortcut: no max connection age; Entra tokens are checked only at connect, so long-lived connections stay valid. Add a max age if Azure starts dropping old sessions.`

- [ ] **Step 1: Write the failing tests** in `postgres_tests.rs`:

```rust
async fn backend_pid(a: &PostgresAdapter) -> i32 {
    let (c, _p) = a.connect_client().await.unwrap();
    c.query_one("SELECT pg_backend_pid()", &[]).await.unwrap().get(0)
}
#[tokio::test] async fn pool_reuses_connections()
// two sequential backend_pid calls on one adapter return the same pid
#[tokio::test] async fn pool_discards_closed_connection()
// pid1 = backend_pid; from a second adapter run SELECT pg_terminate_backend(pid1);
// short sleep; pid2 = backend_pid(first adapter); assert_ne!(pid1, pid2) and the query succeeded
#[tokio::test] async fn pool_reuses_connection_after_rolled_back_transaction()
// open client, start transaction, run a failing statement, drop without commit;
// next connect_client: SELECT 1 succeeds and `SHOW transaction_isolation` / `SELECT txid_current_if_assigned()` is NULL
```
Mark them like the existing real-DB tests.

- [ ] **Step 2:** `make backend-test-postgres` → `pool_reuses_connections` FAILS (different pids).
- [ ] **Step 3:** Implement as in Interfaces. Dropping a `tokio_postgres::Transaction` already queues `ROLLBACK` before the client is reused. If the third test still fails, add a `poisoned: bool` set by callers is **not** wanted; instead run `client.batch_execute("ROLLBACK")` (a no-op warning outside a transaction) right after popping an idle client in `connect_client`, and ledger the ruling.
- [ ] **Step 4:** `cargo test -p loan-service && make backend-test-postgres && cargo clippy -p loan-service --all-targets -- -D warnings` → pass.
- [ ] **Step 5: Commit** `perf(loan): reuse Postgres connections within the connection limit`

### Task 5: Signed-token test through the router

**Files:**
- Modify: `backend/crates/loan-service/src/presentation.rs` (`contract_tests` module)

- [ ] **Step 1: Write the test.**

```rust
#[tokio::test]
async fn real_verifier_accepts_signed_student_token() {
    // AppState { auth: Arc::new(Auth0JwtVerifier::with_test_jwks()), repo: FakeStore::with_units(1), .. }
    // bearer = crate::auth::test_keys::sign(&claims("s1", &["student"]), KID)
    let (status, body) = request(&app, "GET", "/api/v1/reservations", Some(&bearer), json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!([]));
    // tampered: bearer with last char changed -> 401 "InvalidToken"
}
```
- [ ] **Step 2:** Verify it guards wiring: temporarily build `AppState` with `Auth0JwtVerifier::from_env()` → test FAILS (503); revert.
- [ ] **Step 3:** `cargo test -p loan-service` → pass.
- [ ] **Step 4: Commit** `test(loan): cover the real token verifier through the router`

### Task 6: Close out

- [ ] `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` from `backend/` → pass.
- [ ] Update `mem:architecture/persistence` in place if it describes per-request connections (read `memory_maintenance` first); run `serena memories check`. Commit `docs: record outbox and connection fixes`.
