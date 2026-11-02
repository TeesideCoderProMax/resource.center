# Decisions and open issues
Reviewed 2026-11-01; accepted design is not proof of full implementation.
- Accepted: Campus Device Loan; Next.js; Rust/Tokio/Axum; Auth0; Loan-owned PostgreSQL/outbox/publisher; independent Notifications with Cosmos/Resend; clean ports/adapters. ADR 0002 records Rust; ADR 0003 records managed Container Apps ingress and web hosting.
- Test platform choices: France Central, Container Apps Consumption, private PostgreSQL, Cosmos free tier/shared 400 RU/s, Service Bus Basic, Standard ACR. Sources: infra/platform, infra/workloads and docs/cloud/phase-1-handoff.md.
- User's free-tier-first constraint remains; do not infer that scale-to-zero makes all resources free. Only Test has stored deployment evidence; Staging/Production templates are not deployed proof.
- User corrected assessment to CIS3039-N on 2026-11-01. Source criteria: `mem:assessment/cloud_module_criteria`; gaps: docs/assessment/2026-11-01-cloud-backend-alignment.md.
- ADR 0004 proceeds in four isolated sequential chats. Current phase results, handoffs and next work: `mem:backend/plan`; do not infer deployment from local checks.
- Open: configure/verify Auth0 tenant (claims Action, web secrets) and live email as required by approved MVP; confirm tutor approval status for the prepared Stage 2 diagram; prove commit-driven Test deployment and actual promotion gates; finish report/media.
- Notification recovery and retention/replay rules: docs/backend/notification-recovery.md. Local concurrency tests pass; real Cosmos and deployed recovery checks remain open. No exactly-once email claim.
