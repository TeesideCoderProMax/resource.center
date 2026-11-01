# Phase 1 — Loan rules and atomic transactions

Status: **implemented and verified locally, 2026-11-01**. Tutor approval status is unknown; deployment remains pending. See [handoff](phase-1-handoff.md). Original work/check list follows. One isolated implementation chat. Follow the shared rules and handoff format in `docs/backend/README.md`.

## Goal and entry context

Make Loan behaviour match the brief and make business rules testable outside PostgreSQL. Preserve the running architecture and transaction guarantees.

Read ADR 0004, the CIS3039 criteria and current alignment review. Serena: `architecture/clean_architecture`, `architecture/persistence`, `architecture/service_boundaries`, `architecture/testing`, `assessment/cloud_module_criteria`. Inspect `backend/crates/loan-service`, `rc-shared`, migrations and existing tests. No earlier new-plan phase is required.

## Work

1. Build a requirement-to-use-case map for catalogue, reserve, subscribe, collect and return. State actor, input, state change, error and emitted event. Document actual route names and payloads in `docs/backend/contracts.md`; reconcile `/api/v1/subscriptions` with old documentation. Avoid an unneeded breaking route rename.
2. Finish the Stage 2 annotated Container diagram in `docs/backend/stage-2-plan.md`: requirements, security, fakes, MVP labels for both services and dependencies, and phase/build order. Update C4/deployment diagrams to the actual design. Record tutor feedback/approval status without inventing it.
3. Implement explicit application use cases with domain transitions and narrow transaction-scoped ports. Start with reserve, then subscribe/collect/return. Keep locked reads, decision, state write and outbox write in one transaction. Define consistent locking order across competing operations. Keep HTTP, database clients and vendor errors outside the domain.
4. Assign `due_at` at reservation from a single UTC clock value plus two days; include it in the response and persist it. Collection preserves the date and records collection time. Implement the transition rules in ADR 0004. Define ownership/staff access and duplicate-action responses explicitly.
5. Add an additive versioned migration for existing nullable due dates: preserve valid recorded dates, backfill missing ones from reservation timestamps, inspect problematic rows and add constraints only after validation. Keep seed and schema migration separate. Do not edit a migration already applied as the only upgrade path.
6. Keep collection/return notification requests atomic with Loan state. Return selects all active eligible subscribers, with stable single-recipient IDs and clear duplicate-subscription behaviour. Document the existing v1 envelope and retain compatible consumers.
7. Update fake adapters to use the same domain decisions rather than duplicate unrelated business implementations. Inject clocks/IDs where tests need control. Add only read API contracts needed for later student/staff screens; implement them here if part of the Loan use-case surface.

## Completion checks

- Domain tests prove reservation-time due date, unchanged date on collection, role/ownership checks, invalid/repeated transitions and duplicate subscriptions.
- Real PostgreSQL tests prove one winner for the last unit, consistent counts, no double return, multi-subscriber outbox creation and rollback after a forced failure between writes. A role rejection before a transaction is not an atomic rollback test.
- Migration checks cover a fresh database and representative existing rows; existing dates survive, null dates follow the documented rule and no destructive reset occurs.
- Route/contract tests verify due-date response and consistent error codes; event payloads remain compatible with the existing worker.
- Run `make backend-fmt`, `make backend-lint`, `make backend-test`; add a documented real-database test command if none exists. Record database checks as blocked if the required service is unavailable.

## Scope and handoff

Do not implement worker leases, browser UI or deployment gates here. Save `docs/backend/phase-1-handoff.md`, `contracts.md`, `stage-2-plan.md`, updated diagrams, migration details and test evidence. Phase 2 needs stable event fields/IDs, actual transaction/outbox behaviour and known failures. Phase 3 needs API routes, due-date semantics and the user/staff read contract.

## Starting prompt

```text
Implement phase 1 of the Resource Center backend plan in /home/cabozzz/dev/resource.center. Read AGENTS.md, activate this Serena project, then read backend/plan, docs/backend/README.md, docs/backend/phase-1-loan-core.md and ADR 0004. Follow the shared session rules. Implement the Loan use cases, reservation-time due date, atomic transaction ports and meaningful tests; prepare the Stage 2 annotated diagram and API/event contract. Preserve unrelated work and existing data. Complete the phase-1 handoff, update current memories and AI_LOG.md, and report any checks or tutor inputs still pending. Do not start phase 2.
```
