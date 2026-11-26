# Auth Verifier and Queue Consumer Ports Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put a port in front of `Auth0JwtVerifier` (loan-service) and `ServiceBusRestConsumer` (notifications-worker) so presentation code and the receive loop depend on traits and tests can inject fakes.

**Architecture:** Each port lives in its crate's `application.rs`, following the existing port style of that crate (loan-service: `BoxFuture` traits like `LoanRepositoryPort`; notifications-worker: `#[async_trait]` traits like `NotificationJobStorePort`). The concrete adapters stay in infrastructure and implement the ports. `main.rs` remains the composition root that constructs the adapters; the worker's receive loop moves out of `run_worker` into an application function generic over the consumer port.

**Tech Stack:** Rust, Tokio, Axum, `async-trait`, existing `cargo test`.

**Spec:** User request of 2026-11-18 (AI_LOG entry 124). Constraints from `AGENTS.md` (Clean Architecture: application independent of vendor SDKs; infrastructure implements ports; composition roots inject adapters).

## Global Constraints

- No new dependencies.
- `ServiceBusRestConsumer` and `Auth0JwtVerifier` are still constructed in each crate's `main.rs`.
- Application modules must not import from `infrastructure` or `auth`.
- No behaviour change: same HTTP status codes/error bodies, same loop timings (`wait_secs` 5 in drain-once / 20 otherwise, 5 s back-off on poll error, 10 s renew interval and 10 s renew timeout), same log events.
- Existing adapter tests in `auth.rs` (real JWKS/test-key verification) stay; they test the adapter, not the presentation.

## Review Focus

1. Poll error in drain-once mode must return `Err` (CI drain job relies on it) — pinned by `drain_once_returns_error_on_poll_failure`.
2. Malformed message body must be abandoned, not completed — pinned by `malformed_message_is_abandoned`.
3. `complete_message` failure must propagate as `Err` (today `?`), while `abandon_message` failure is ignored — pinned by `complete_failure_propagates`.
4. Auth error variants still map to the same status codes after swapping to a fake (401 missing token, 503 keys unavailable / not configured) — pinned by `fake_verifier_errors_map_to_status`.
5. Health/ready `auth0Configured` still comes from the verifier — pinned by `health_reports_verifier_configuration`.

---

### Task 1: Loan `AccessTokenVerifierPort`

**Files:**
- Modify: `backend/crates/loan-service/src/application.rs` (add port + move `AuthVerifyError` here)
- Modify: `backend/crates/loan-service/src/auth.rs` (impl port; `pub use crate::application::AuthVerifyError`)
- Modify: `backend/crates/loan-service/src/presentation.rs:14-20,373-381` and its tests (`470`, `569`, `601`)
- Modify: `backend/crates/loan-service/src/main.rs:186`
- Modify: `backend/crates/loan-service/src/test_support.rs` (add `FakeVerifier`)

**Interfaces:**
- Produces, in `application.rs`:
  ```rust
  pub trait AccessTokenVerifierPort: Send + Sync {
      fn is_configured(&self) -> bool;
      fn verify_bearer<'a>(&'a self, auth_header: Option<&'a str>)
          -> BoxFuture<'a, Result<AuthenticatedActor, AuthVerifyError>>;
  }
  ```
  `AuthVerifyError` moves unchanged (same variants and `#[error]` text) into `application.rs`; `auth.rs` and `infrastructure.rs:890` re-export it so existing paths compile.
- `AppState.auth: Arc<dyn AccessTokenVerifierPort>`; `authenticate_request(verifier: &dyn AccessTokenVerifierPort, headers: &HeaderMap)`.
- `test_support::FakeVerifier { configured: bool, result: Result<AuthenticatedActor, fn() -> AuthVerifyError> }` — or simpler: `FakeVerifier::actor(subject, roles: &[Role])` and `FakeVerifier::failing(fn() -> AuthVerifyError)`; implementer picks, since `AuthVerifyError` is not `Clone`. `is_configured()` returns `true` for `actor`, configurable for `failing`.

- [ ] **Step 1: Write failing tests in `presentation.rs` tests module**

```rust
#[tokio::test]
async fn fake_verifier_errors_map_to_status() {
    // MissingBearerToken -> 401, KeysUnavailable -> 503, NotConfigured -> 503
    // GET /api/v1/reservations with AppState { auth: Arc::new(FakeVerifier::failing(..)), .. }
}
#[tokio::test]
async fn health_reports_verifier_configuration() {
    // FakeVerifier with configured=false -> body["auth0Configured"] == false
}
```
Use the existing request helper in the tests module; assert on the same status codes `map_auth_error` produces today (read it at `presentation.rs:383+` and copy the values).

- [ ] **Step 2:** `cargo test -p loan-service fake_verifier` → FAIL (`FakeVerifier`/port not defined).
- [ ] **Step 3:** Add the port and move `AuthVerifyError`; `impl AccessTokenVerifierPort for Auth0JwtVerifier` delegating to the existing inherent methods (`Box::pin(self.verify_bearer(h))` — rename inherent if it collides, or call `Auth0JwtVerifier::verify_bearer(self, h)`). Change `AppState`, `authenticate_request`, and `main.rs:186` to `Arc::new(Auth0JwtVerifier::from_env())`.
- [ ] **Step 4:** Replace `Auth0JwtVerifier::with_test_jwks()` / `from_env()` in presentation tests with `Arc::new(FakeVerifier::actor(..))`; drop the signed `token(..)` helper there if nothing else uses it (keep `test_keys` for `auth.rs` tests). Any test needing different actors per request: `FakeVerifier` maps the bearer string to the subject (e.g. `"Bearer owner:student"`) — only add this if an existing test sends two identities.
- [ ] **Step 5:** `cargo test -p loan-service && cargo clippy -p loan-service --all-targets -- -D warnings` → all pass.
- [ ] **Step 6: Commit** `refactor(loan): put access-token verification behind a port`

### Task 2: Worker `QueueConsumerPort` and testable receive loop

**Files:**
- Modify: `backend/crates/notifications-worker/src/application.rs` (port, move `PeekLockedMessage`, `run_consumer_loop`, tests)
- Modify: `backend/crates/notifications-worker/src/infrastructure.rs:250-400` (impl port; re-export `PeekLockedMessage`)
- Modify: `backend/crates/notifications-worker/src/main.rs:66-237` (keep construction; call loop)

**Interfaces:**
- Produces, in `application.rs` (same `#[async_trait]` + `clippy::double_must_use` allow as `NotificationJobStorePort`):
  ```rust
  pub trait QueueConsumerPort: Send + Sync {
      async fn receive_peek_lock(&self, wait_secs: u64) -> Result<Option<PeekLockedMessage>, String>;
      async fn renew_lock(&self, lock_location: &str) -> Result<(), String>;
      async fn complete_message(&self, lock_location: &str) -> Result<(), String>;
      async fn abandon_message(&self, lock_location: &str) -> Result<(), String>;
  }
  pub async fn run_consumer_loop<C, S, E, F, Fut>(
      consumer: &C, processor: &NotificationProcessor<S, E>, drain_once: bool, shutdown: F,
  ) -> Result<usize, String>   // processed_count
  where C: QueueConsumerPort + ?Sized, F: Fn() -> Fut, Fut: Future<Output = ()>,
        S: NotificationJobStorePort, E: EmailDeliveryPort;
  ```
  Match `wait_secs`' actual type in `ServiceBusRestConsumer::receive_peek_lock` (line 298). `shutdown` is called each place `shutdown_signal()` is called today; `main.rs` passes `shutdown_signal`, tests pass `std::future::pending`.
- Body = the current loop from `main.rs` lines ~136–234 moved verbatim, with `consumer.complete_message(..).await?` and drain-once poll errors returned as `Err(String)`. `main.rs` maps `Err` into its boxed error.

- [ ] **Step 1: Write failing tests** in `application.rs` tests, with a `FakeConsumer { inbox: Mutex<VecDeque<Result<Option<PeekLockedMessage>, String>>>, completed: Mutex<Vec<String>>, abandoned: Mutex<Vec<String>>, fail_complete: bool }` reusing `InMemoryCosmosStore` and `CountingFakeSender`:

```rust
#[tokio::test] async fn drain_once_completes_valid_message_and_stops_on_empty()
// inbox: [Ok(Some(msg(valid json of sample_request("e1"), "lock-1"))), Ok(None)]
// assert Ok(1), completed == ["lock-1"], abandoned empty, sender sent 1
#[tokio::test] async fn malformed_message_is_abandoned()
// body "not json" -> abandoned == ["lock-1"], completed empty
#[tokio::test] async fn drain_once_returns_error_on_poll_failure()
// inbox: [Err("boom")] -> Err containing "boom"
#[tokio::test] async fn complete_failure_propagates()
// fail_complete = true -> Err
```
Check `process_until_settled` on a malformed body actually returns `Abandon` (`application.rs:789` test says malformed does not send); if it returns `Complete`, change the assertion to match current behaviour — this is a refactor, not a behaviour change.

- [ ] **Step 2:** `cargo test -p notifications-worker run_consumer` / the test names → FAIL (not defined).
- [ ] **Step 3:** Add port, move `PeekLockedMessage`, `impl QueueConsumerPort for ServiceBusRestConsumer` (delegate to existing methods), move the loop into `run_consumer_loop`. In `main.rs::run_worker`, keep `ServiceBusRestConsumer::new(..)` and replace the loop with `run_consumer_loop(consumer.as_ref(), &processor, drain_once, shutdown_signal).await?`.
- [ ] **Step 4:** `cargo test -p notifications-worker && cargo clippy -p notifications-worker --all-targets -- -D warnings` → pass (Cosmos-gated tests skip as before).
- [ ] **Step 5: Commit** `refactor(worker): put Service Bus consumer behind a port and test the receive loop`

### Task 3: Record status

- [ ] Update the relevant memory (`architecture/clean_architecture` or `backend/plan`, per `memory_maintenance`) in place: both adapters now sit behind ports; run `serena memories check`.
- [ ] `cargo test --workspace` from `backend/` → pass. Commit `docs: record auth and queue consumer ports`.
