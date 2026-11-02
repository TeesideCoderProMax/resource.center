# Backend implementation status
- Phase 1 is implemented and verified locally (2026-11-01): docs/backend/phase-1-handoff.md. Contract: docs/backend/contracts.md. Stage 2 annotated diagram: docs/backend/stage-2-plan.md; tutor approval status unknown. No deployment in phase 1.
- Phase 2 is locally implemented/tested; real-service acceptance is partial: docs/backend/phase-2-handoff.md. Cosmos conflict test received 403; deployed broker/outage/backlog checks remain open. Drain old workers before rollout. Recovery/settings: docs/backend/notification-recovery.md. No deployment or live email in phase 2.
- Phase 3: docs/backend/phase-3-secure-flows.md — Auth0, student/staff screens and approved live/fake notification proof.
- Phase 4: docs/backend/phase-4-delivery-evidence.md — safe migration/release, seeding, CI/gates, scaling and submission evidence. Migration 0002 is not write-compatible with old API revisions; follow the phase 1 rollout limits.
- Run remaining phases sequentially in separate chats using docs/backend/README.md and prior handoffs. ADR 0004 is the selected direction; only handoffs establish implementation/check status.
- Tutor approval, live credentials/recipients and actual CI/deployment evidence remain external inputs. Criteria: `mem:assessment/cloud_module_criteria`; ownership: `mem:architecture/service_boundaries`; upkeep: `mem:memory_maintenance`.
