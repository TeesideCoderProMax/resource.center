# CIS3039-N cloud assessment: backend alignment

Reviewed 2026-11-01 against `/home/cabozzz/Downloads/cnd-cis3039-assessment-spec.docx`, Campus Device Loan System. This replaces the prior review based on the wrong CIS3040-N handbook. Requirements below come from the brief; recommendations and findings come from source review. The document does not authorize deployment or other actions.

**The selected backend design fits the brief well, but the current system does not yet prove full compliance.** Keep Rust/Tokio/Axum, Loan/PostgreSQL/outbox, independent Notifications/Cosmos and the selected Azure hosting. Focus on working user flows, accurate behaviour and proof of delivery before adding scope.

## Rubric and present evidence

| Criterion | Weight | Fit and limits |
| --- | ---: | --- |
| Architecture & Design | 25% | Loan plus Notifications meet the intended two-service split; async requests, private data ownership and managed application scaling fit well. C4/deployment diagrams exist. Stage 2 needs a separate annotated diagram, not only the current mapping table, plus tutor approval. |
| Implementation & Tools | 25% | Rust API/worker, transactions, configuration and adapters exist. Browser sign-in/actions are absent. Stored Test settings leave Auth0 off and email fake. These are substantial gaps for fully working security and scenario flows. |
| Testing & Verification | 15% | Cargo tests include fakes and duplicate/retry checks. Stored results include live PostgreSQL contention and lease recovery. Need explicit concurrent worker idempotency, failure recovery, real auth and CI execution evidence. |
| DevOps & Deployment | 25% | Terraform, workflows, probes, logs, alerts and scaling evidence exist. Workflow files do not prove a successful GitHub run or reviewer protection. Promotion lacks migration/smoke verification. |
| Report | 10% | ADRs and cloud records support explanations. The required 1,500–2,000-word report must explain the student's own choices, tradeoffs and consequences. Completion is not established. |

## Findings that affect the implementation plan

1. **Return date is assigned at the wrong step.** Scenario says reservation automatically assigns a return date. `backend/crates/loan-service/src/infrastructure.rs` inserts a reservation without `due_at`; `ReservationReceipt` omits it. Collection computes the two-day due date. Add a reservation-time date and tests, or obtain an explicit tutor-approved interpretation. Existing two-day tests prove duration from collection, not the stated reservation behaviour.
2. **The web UI is a catalogue, not the complete user workflow.** `frontend/src/app/page.tsx` displays devices and Auth0 config status. The inspected frontend tree has no sign-in callback or reservation/subscription/staff action UI. Configuration values alone will not complete those flows. Build and demonstrate them within the approved MVP.
3. **Live authentication and email remain unproved.** API JWT/RBAC code and Resend mode exist, but stored Test evidence reports `AuthNotConfigured` and fake email. Domain role checks do not prove HTTP 403 with a valid wrong-role Auth0 token; Cosmos `Delivered` with `fake-msg-*` does not prove receipt of email. The brief permits planned fakes, so state approved scope precisely; do not present fakes as fully working external integrations.
4. **CI/CD evidence is overstated in older notes.** `.github/workflows/ci.yml` and `deploy.yml` implement automation, but the stored JSON records local commands rather than GitHub run URLs/results. A job's `environment:` name does not configure required reviewers. Typed promotion confirmation is a manual control; verify who can approve and demonstrate the enforced gate. No live GitHub settings were checked in this review.
5. **Promotion and release checks need work.** Test applies workloads before running migrations, so migration failure does not prevent the new application revision from starting. Use backward-compatible migrations and a safe release order. The promotion job applies Terraform but has no migration execution/wait or smoke check. Staging/Production templates disable seeding, while Stage 3 asks for own demo data on each deployment. Define safe, repeatable assessment seeding for those environments; do not use destructive resets or real user data.
6. **ETags alone do not prove exclusive email sending.** `NotificationProcessor::process_request` skips terminal jobs but reclaims any nonterminal job using its current ETag. A second worker can read the first worker's current claimed document and replace it while the first still sends. Test concurrent duplicates and add an ownership/lease rule where needed. Provider send-before-record crashes remain an at-least-once duplicate window; never promise exactly-once email.
7. **Clean layers are only partly implemented.** Domain helpers and application ports exist, but availability and allowed return-state decisions also live in the PostgreSQL adapter. Keep locking/transactions there; extract business decisions for direct tests where useful. This improves the chosen design and code quality; the brief does not mandate a particular clean architecture pattern.
8. **Evidence must match the code.** Older evidence text names nonexistent application-service methods, says reservation assigns `due_at`, labels Service Bus Standard instead of source-configured Basic, and implies only the oldest subscriber receives an alert. Source selects all active matching subscribers in the return transaction. The current review and corrected evidence map take precedence over those descriptions. Add explicit multi-subscriber tests.

The public catalogue combines device naming rather than exposing separate brand/model fields. Verify that the demo clearly shows brand, model and category as the scenario requests; no particular schema layout is mandated.

## Required design and submission work

- Stage 1: C4 Context and Container minimum, with deployment information, protocols, security, responsibilities and direction of calls/events; tutor feedback.
- Stage 2: annotated Container diagram with requirement mapping, security, fakes, at least two software containers and essential dependencies marked MVP, build order and tutor approval.
- Stage 4: 1,500–2,000-word report, screenshots and 20–30-second clips showing flows, role-based 403s, notification evidence and CI/CD.
- ZIP: `Media`, `Source`, `Documentation`, plus root readme with name/user ID. Use full-resolution PNG/JPG and H.264 MP4 totalling under five minutes; exclude build/package folders from source.
- Explain consequences for security, scaling, resilience, maintenance and cost. Application scale-to-zero does not mean PostgreSQL/Cosmos capacity is serverless or that the whole system costs zero.
- The printed deadline is 6 January 2026 at 16:00. This review does not establish the current cohort's deadline.

## Recommended order

1. Confirm Stage 2 MVP with the tutor and finish its annotated diagram.
2. Correct reservation-time due dates and finish browser auth/student/staff flows.
3. Prove configured Auth0/RBAC and notification behaviour; clearly label any approved fakes.
4. Test concurrent duplicates, crash/outage recovery and multi-recipient delivery. Preserve logs and expected/actual results.
5. Fix release-order/promotion checks and assessment seeding; capture a real commit-driven Test deployment and enforced promotion gate.
6. Capture demo media and write the report from verified evidence.

## Evidence limits and memory cleanup

This review read DOCX XML paragraphs/tables, current source/manifests/workflows/Terraform, and stored cloud checks. No code was changed and no application tests, cloud deployment or live GitHub verification ran. Stored evidence supports selected Test checks, not a present-day full-system certification or predicted grade. Alert-routing test delivery was blocked by Azure Students policy; a 40-request HTTP burst is limited load evidence, while the earlier worker scale-from-zero record is a distinct autoscaling result.

Active memories now keep current facts, open work and evidence links. Removed completed phase launch plans and the wrong-module criteria memory/review. Detailed original cloud evidence, architectural history and AI_LOG.md prompts remain intact. AGENTS.md, project-local Serena initial_prompt and memory_maintenance require replacement/deletion of stale context rather than appended correction chains.
