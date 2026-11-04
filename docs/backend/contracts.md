# Loan API and notification contract

Implemented locally on 2026-11-01 in backend phase 1. Not deployed by this phase.
All API fields use camelCase. Times use UTC RFC 3339. IDs use UUIDs unless stated otherwise.
The API accepts `x-correlation-id`, or creates one, and returns it as a response header.

## Requirement to use case

| Requirement / use case | Actor and input | State change | Success / errors | Event |
| --- | --- | --- | --- | --- |
| Catalogue | Public; no input | None | 200; storage failure 500 | None |
| Reserve | Student or staff; modelId | Available unit → RESERVED; count −1; create reservation with due date | 201; missing model 404; no unit 409; blank model 400 | None |
| Subscribe | Student or staff; modelId | Create or reuse ACTIVE subscription while count is zero | 201; missing model 404; available units 409 | None |
| Collect | Staff; reservation UUID | RESERVED → COLLECTED; record collection time; retain due date | 200; missing reservation 404; invalid/repeated state 409 | Collection confirmation |
| Return | Staff; reservation UUID | COLLECTED → RETURNED; unit → AVAILABLE; count +1; active subscriptions → NOTIFIED | 200; missing reservation 404; invalid/repeated state 409 | Return confirmation plus one availability request per distinct recipient |
| List reservations | Student or staff; no input | None | 200; role failure 403 | None |

Actors and recipient addresses come from verified token claims, never from request body fields. Since phase 3, roles come only from `AUTH0_ROLES_CLAIM` (default `https://resource-center.campus/roles`) and the recipient address only from `AUTH0_EMAIL_CLAIM` (default `https://resource-center.campus/email`). An Auth0 post-login Action must add both to the access token. A token without a usable email claim is rejected with 401; there is no fallback address. The issuer is `AUTH0_ISSUER`, or `https://<AUTH0_DOMAIN>/` when only the domain is set.
Students can reserve and subscribe for themselves. Staff can do the same and can collect/return any reservation.
A student cannot collect or return even their own reservation. The read use case filters by token subject for students; staff can read all owners.

## Routes and payloads

- `GET /api/v1/catalogue`: public. Returns `{items: [...], correlationId, auth0Configured}`. Each item has `id`, `name`, `category`, `description`, `loanDurationDays`, `totalUnits`, `availableUnits`, nullable `imageUrl` and `imageAlt`, and `specifications` (an ordered array of `{label, value}` strings). `loanDurationDays` is the maximum allowed period, not the chosen reservation period. The existing `name` holds the brand/model label; no separate brand field exists.
- `POST /api/v1/reservations`: body `{"modelId":"dell-latitude-7440","loanDurationDays":7}`. The chosen period must be an integer from 1 to the locked model maximum (at most 14); missing, fractional or invalid periods return 400 `InvalidInput`. Returns `reservationId`, `modelId`, `unitId`, `status`, `reservedAt`, **`dueAt`**, `correlationId`.
- `POST /api/v1/subscriptions`: body `{"modelId":"dell-latitude-7440"}`. Returns `subscriptionId`, `modelId`, `status`, `createdAt`. Repeated active subscription returns the original ID/time with 201 and refreshes the trusted email. This is the existing route; `/api/v1/availability-subscriptions` in older documents is not an alias.
- `POST /api/v1/reservations/{id}/collect`: no body needed. Returns `reservationId`, `unitId`, `status`, `collectedAt`, `dueAt`, `outboxEventId`.
- `POST /api/v1/reservations/{id}/return`: no body needed. Returns `reservationId`, `unitId`, `modelId`, `status`, `returnedAt`, `outboxEventIds`.
- `GET /api/v1/reservations`: protected. Returns a JSON array with the owner's rows, or all owners for staff. Each row has `reservationId`, `modelId`, `unitId`, `studentSub`, `studentEmail`, `status`, `reservedAt`, `dueAt`, nullable `collectedAt`/`returnedAt`, and `correlationId`. PostgreSQL returns at most 200 rows, newest first (`reservedAt`, then ID). This is the initial screen contract; no paging or history export yet.

Reservation captures one UTC time at microsecond precision. `dueAt = reservedAt + the chosen whole-day period`. Collection does not extend it, even if collection occurs late. Cancellation, expiry, renewal and per-student loan limits are outside phase 1. Repeated reserve calls can create separate loans if units remain; no request-idempotency key exists.

Return selects every ACTIVE subscription for the model under the model lock. Active status is the current eligibility rule; it does not impose a new per-student loan limit. Distinct email addresses receive separate events. Two subjects with the same email receive one availability event; all their active subscriptions become NOTIFIED. A student can subscribe again in a later zero-availability cycle. Past subscriptions remain stored.

## Errors

Application errors use `{error, message, correlationId}` and the correlation header.

| HTTP status | error |
| --- | --- |
| 400 | `InvalidInput` |
| 401 | `Unauthorized`, `InvalidToken` |
| 403 | `ForbiddenRole` |
| 404 | `NotFound` |
| 409 | `NoAvailableUnits`, `UnitsCurrentlyAvailable`, `InvalidTransition` |
| 500 | `InternalError` |
| 503 | `AuthNotConfigured`, `AuthUnavailable` (no usable JWKS), or readiness `DatabaseUnreachable` |

Reservation JSON extraction errors use the 400 `InvalidInput` envelope. For other routes, Axum handles malformed JSON, wrong content type and invalid UUID path syntax before the handler; these extractor errors still use Axum's default response format. Phase 1 does not claim a single envelope for these transport errors. Missing Auth0 config fails closed with 503. Auth error messages are fixed text; the detail goes only to logs. The web app calls these routes through its server-side `/api/loan/*` proxy (see phase 3 handoff). Test JWTs prove local signature/claim checks, not live Auth0 setup or sign-in.

## Transaction and storage contract

`LoanRepositoryPort` is the HTTP-facing use-case interface. Its implementation in `application.rs` runs over `LoanStorePort`. `LoanTransactionPort` exposes locked reads and writes without SQL or vendor types. The adapter starts one transaction, runs the application callback, then commits only its successful result. Error or cancellation drops the transaction and rolls it back.

All model-changing use cases lock the model first. Collection/return read only the immutable model reference before that lock, then reload and lock the reservation and unit. The order is model → reservation/unit → active subscriptions → outbox. All decisions use locked state. Reserve and subscribe share the model lock with return, so a subscription cannot miss a concurrent return after it commits. No remote call occurs inside the transaction. PostgreSQL connection use remains bounded.

The fake adapter uses a locked copy and only saves it on success. It runs the same application/domain decisions. It proves local logic; only the PostgreSQL tests prove SQL locks and rollback.

## NotificationRequestV1 (unchanged)

```json
{
  "eventId": "evt-<UUID-v5>",
  "type": "DeviceCollectionConfirmationRequested",
  "version": 1,
  "occurredAt": "2026-11-01T12:00:00Z",
  "correlationId": "request-correlation-id",
  "recipient": "student@example.invalid",
  "deviceModel": "Dell Latitude 7440"
}
```

Supported types: `DeviceCollectionConfirmationRequested`, `DeviceReturnConfirmationRequested`, `DeviceAvailabilityNotificationRequested`. Optional `syntheticProbe: true` is reserved for existing test probes; ordinary use cases omit it. Required string fields must be nonblank; `rc-shared` rejects unknown type/version. Current validation does not fully validate email syntax.

Event IDs use UUID v5, namespace OID, over `<transition-key>:<recipient>`, with `evt-` prefix. Transition keys are `collect:<reservation UUID>`, `return:<reservation UUID>` and `availability:<reservation UUID>`. IDs are stable for one transition/recipient and differ across later returns. State changes, subscription consumption and all event rows commit together. Invalid/repeated transitions emit nothing.

Publisher uses `eventId` as Service Bus MessageId and passes correlation identity. Consumers retain the v1 schema; no worker change or Loan callback is needed. At-least-once publication and send-before-record failures can duplicate delivery. Phase 2 must address lease ownership/recovery. This contract does not promise exactly-once email.

## Migration 0002 and safe rollout

`0001_initial_loan_schema.sql` stays unchanged. The runner records `0002_reservation_due_dates`, serializes migrations with an advisory transaction lock, and applies it once. Seed SQL remains a separate file/optional step.

Before upgrade, inspect rows with this query using the migration identity:

```sql
SELECT reservation_id, reserved_at, due_at
FROM loan.reservations
WHERE due_at IS NULL OR NOT isfinite(reserved_at)
   OR NOT isfinite(due_at) OR due_at < reserved_at;
```

The migration aborts on nonfinite dates or a recorded due date before reservation. An operator must inspect and correct such data; no automatic destructive repair occurs. It preserves all valid non-null dates, including dates set by the old collection rule. It fills only null dates with `reserved_at + 48 hours`, then validates finite/order checks and NOT NULL. It replaces all-status subscription uniqueness with a partial unique index for ACTIVE rows only.

Old API revisions omit due dates and use the old subscription conflict target. **They are not write-compatible with migration 0002.** Before deployment, pause Loan writes, drain old API revisions, apply the migration, then activate the matching image and check it. Do not send old and new write traffic together. A rollback to the old image requires an explicit reviewed schema rollback; do not drop constraints or reset data as an automatic recovery. Phase 4 must enforce this release order. This phase does not deploy or alter the cloud database.

Validation and next inputs: [phase 1 handoff](phase-1-handoff.md).

## Catalogue and selectable periods (2026-11-04)

Migration 0003 adds optional image metadata and a validated JSONB specification
array. It replaces the fixed two-day model limit with 1–14 days and sets existing
model maxima to 14. It never changes recorded reservation due dates. Migration
0002's 48-hour backfill remains historical upgrade behaviour for missing dates.

The reservation use case reads the maximum under the model lock, validates the
chosen period, and computes dueAt from the server reservation time plus that
many 24-hour days. Browser dates are estimates. Existing collect/return and
notification flows use the saved due date.

Deploy the matching API and frontend together using the maintenance release
process. Old frontends omit the required period; old APIs always calculate two
days. Neither is compatible with the new reservation contract. See
`docs/backend/catalogue-and-loan-periods.md` for release evidence.
