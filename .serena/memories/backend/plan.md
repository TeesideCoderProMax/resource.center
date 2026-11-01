# Backend implementation status
- Phase 1 is implemented and verified locally (2026-11-01): docs/backend/phase-1-handoff.md. Contract: docs/backend/contracts.md. Stage 2 annotated diagram: docs/backend/stage-2-plan.md; tutor approval status unknown. No deployment in phase 1.
- Phase 2 is next and ready for local work: docs/backend/phase-2-notifications.md. Preserve v1 events; implement owned leases, retry/recovery and concurrent duplicate tests. Do not treat existing ETags as exclusive send ownership.
- Phase 3: docs/backend/phase-3-secure-flows.md — Auth0, student/staff screens and approved live/fake notification proof.
- Phase 4: docs/backend/phase-4-delivery-evidence.md — safe migration/release, seeding, CI/gates, scaling and submission evidence. Migration 0002 is not write-compatible with old API revisions; follow the phase 1 rollout limits.
- Run remaining phases sequentially in separate chats using docs/backend/README.md and prior handoffs. ADR 0004 is the selected direction; only handoffs establish implementation/check status.
- Tutor approval, live credentials/recipients and actual CI/deployment evidence remain external inputs. Criteria: `mem:assessment/cloud_module_criteria`; ownership: `mem:architecture/service_boundaries`; upkeep: `mem:memory_maintenance`.
