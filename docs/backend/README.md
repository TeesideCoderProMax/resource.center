# Backend implementation: four isolated chat sessions

Status: **phases 1–3 implemented locally; live acceptance for phases 2–3 remains open; phase 4 implemented locally; live delivery and submission evidence partial**. Tutor approval status is unknown; deployment remains pending. Created 2026-11-01 from the user's request and the CIS3039 assessment review. Updated design: `docs/architecture/adr/0004-backend-use-cases-and-recovery.md`. Preserve the selected stack and existing data.

| Phase | Result | Depends on |
| --- | --- | --- |
| [1: Loan rules and transactions](phase-1-loan-core.md) | Testable use cases, correct due dates, atomic persistence and documented API/event contract | Current repository |
| [2: Reliable notifications](phase-2-notifications.md) | Exclusive active claims, bounded retry and recoverable delivery | Phase 1 contract and handoff |
| [3: Secure user flows](phase-3-secure-flows.md) | Auth0/RBAC plus working student/staff flows and notification proof | Phases 1–2 handoffs |
| [4: Delivery and assessment evidence](phase-4-delivery-evidence.md) | Safe release/promotion, CI/scaling proof and submission evidence | Phases 1–3 handoffs |

Run one phase per new chat, in this order. Isolated means no dependence on chat history; it does not mean concurrent changes or four separate deployments. No chats are created by this planning request. Each phase contains a copyable starting prompt.

## Rules for every session

1. Work in `/home/cabozzz/dev/resource.center`, never the teeside lab. Read AGENTS.md, Serena initial instructions once per conversation, activate this project, then read core, selected_baseline, project/decisions and the phase-linked memories.
2. Log the exact new prompt in AI_LOG.md. Read this index, ADR 0004, the selected phase and prior handoffs. Inspect current source and working-tree changes; preserve unrelated work. Do not rely on chat history or assume uncommitted files are yours to overwrite.
3. Verify each prerequisite before dependent work. If a prior phase is partial, finish independent work and state the specific dependency that blocks the rest. Do not silently broaden the phase or mark a skipped check passed.
4. These files are plans. The later instruction to implement a phase authorizes its normal scoped code/documentation work. Use the actual session's authority for external operations; do not infer approval for paid environment expansion, sending email, commit/push or production deployment from this plan. Discover existing config before asking for missing inputs; never print secrets.
5. Keep changes small and preserve deployment compatibility. Tests must prove requirements and failure behaviour. Run relevant Makefile checks; full make check at integration/release points. Record local/fake, real service, CI and live environment evidence separately.
6. Save the handoff below, update current design/assessment status and finish AI_LOG.md. Replace stale memory claims; do not append history chains. Reduce completed phase memory to its handoff/current open issues; remove its launch instructions from active memory. Keep detailed phase records in docs. Run serena memories check and git diff --check.

## Handoff required from every phase

Create `docs/backend/phase-N-handoff.md` only after work occurs. Include:

- Date, phase status (complete/partial/blocked), base branch and commit if any, plus relevant uncommitted paths.
- Requirements addressed and actual behaviour changes, API/event/schema contracts and compatibility rules.
- Changed files and migrations; exact commands with outcomes and evidence locations. Separate tests from proposals and prior results.
- Deployment status per environment; configured fake/live modes; secret names/locations only.
- Remaining defects, external blockers, approved decisions and next-phase inputs. List safe recovery or migration limits.
- The next phase's ready/not-ready verdict, with reasons. Never call absent tutor approval, missing credentials or unrun live checks complete.

## Coverage

- Architecture/design 25%: ADR, updated C4/deployment and Stage 2 annotated plan in phase 1; diagrams stay current through phase 4.
- Implementation 25%: Loan in phase 1, Notifications in phase 2, complete secure user flows in phase 3.
- Testing 15%: unit/integration/concurrency/fakes in phases 1–3; CI proof and final regression in phase 4.
- DevOps 25%: safe migration/release, actual gates, observability/IaC/scaling evidence in phase 4.
- Report 10%: record reasons/tradeoffs each session; phase 4 supplies a factual evidence pack and supports the student's own report.

Tutor Stage 2 approval is an assessment prerequisite to record, not an approval this agent can grant. Finish the reviewable diagram/plan before requesting feedback; continue local work allowed by the user's instruction and accurately mark approval pending.

## Current result

Phases 1–3 code and local checks are done: [phase 1](phase-1-handoff.md), [phase 2](phase-2-handoff.md), [phase 3](phase-3-handoff.md). Live Auth0, live email, Cosmos access and deployment remain open. Phase 4 release/tooling results and remaining limits: [handoff](phase-4-handoff.md). No further phase is assumed.
