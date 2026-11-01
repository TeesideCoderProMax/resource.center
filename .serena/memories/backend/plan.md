# Backend implementation plan
User requested updated design and four isolated implementation chats on 2026-11-01. Status: planned, not executed.
- Selected direction: docs/architecture/adr/0004-backend-use-cases-and-recovery.md. Keep the stack; use application-owned transaction scopes, reservation-time due dates, explicit notification lease ownership and safe release ordering.
- Canonical plan/session rules: docs/backend/README.md. Run phases sequentially; no reliance on chat history and no concurrent shared-state work.
- Phase 1: docs/backend/phase-1-loan-core.md — Loan use cases, atomic transaction ports, due dates, migration/tests, API/event contract and Stage 2 annotated diagram.
- Phase 2: docs/backend/phase-2-notifications.md — lease ownership, retry/recovery, concurrent duplicate handling and provider limits.
- Phase 3: docs/backend/phase-3-secure-flows.md — Auth0/RBAC, minimal user/staff UI, live/fake notification evidence.
- Phase 4: docs/backend/phase-4-delivery-evidence.md — migration-before-activation, safe promotion/seeding, real CI/gate/scaling evidence and submission support.
- Each phase file has a starting prompt, inputs, work, checks and outputs. Read prior docs/backend/phase-N-handoff.md files; they do not exist until work occurs. A missing/partial handoff is not a completed dependency.
- Future implementation needs the session's actual authority for external changes; this planning request performs none. Tutor approval, credentials, live recipients and CI access remain explicit evidence inputs.
- Keep actual implementation status in architecture/evidence memories until a phase changes it. After completion, replace active launch guidance with concise result/next-work links under `mem:memory_maintenance`.
- Rubric: `mem:assessment/cloud_module_criteria`; current gaps: `mem:assessment/requirements`; stored deployment evidence: `mem:cloud/plan`.
