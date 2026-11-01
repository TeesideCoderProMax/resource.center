# Decisions and open issues
Reviewed 2026-11-01; accepted design is not proof of full implementation.
- Accepted: Campus Device Loan; Next.js; Rust/Tokio/Axum; Auth0; Loan-owned PostgreSQL/outbox/publisher; independent Notifications with Cosmos/Resend; clean ports/adapters. ADR 0002 records Rust; ADR 0003 records managed Container Apps ingress and web hosting.
- Test platform choices: France Central, Container Apps Consumption, private PostgreSQL, Cosmos free tier/shared 400 RU/s, Service Bus Basic, Standard ACR. Sources: infra/platform, infra/workloads and docs/cloud/phase-1-handoff.md.
- User's free-tier-first constraint remains; do not infer that scale-to-zero makes all resources free. Only Test has stored deployment evidence; Staging/Production templates are not deployed proof.
- User corrected assessment to CIS3039-N on 2026-11-01. Source criteria: `mem:assessment/cloud_module_criteria`; gaps: docs/assessment/2026-11-01-cloud-backend-alignment.md.
- Plan requested 2026-11-01: implement ADR 0004 in four isolated sequential chats (`mem:backend/plan`). This is selected design direction, not completed code or deployment. Preserve current status until each handoff supplies evidence.
- Open: implement browser sign-in/actions; configure/verify Auth0 and live email as required by approved MVP; assign due date at reservation (current code sets it at collection); complete Stage 2 diagram/tutor approval; prove commit-driven Test deployment and actual promotion gates; finish report/media.
- Other design risks: business rules still occur in SQL adapters; concurrent notification claims need tests; define retention/replay and provider idempotency. No exactly-once email claim.
