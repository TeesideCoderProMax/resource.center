# Selected baseline
Checked against source and ADRs 2026-11-01.
- Next.js → Azure Container Apps managed HTTPS → Rust/Tokio/Axum Loan API → private PostgreSQL.
- PostgreSQL transactional outbox → Loan-owned scheduled Rust/Tokio publisher → Azure Service Bus → independent Rust/Tokio Notifications → private Cosmos DB for NoSQL and Resend adapter.
- Loan owns business state and recipients. Notifications owns delivery state; no cross-service database access or ordinary request reconstruction by Loan callbacks.
- Auth0 OIDC/PKCE sign-in and JWT signature/issuer/audience/expiry plus student/staff RBAC are required. API validation exists; browser sign-in and complete live auth remain open.
- Container Apps Consumption hosts web/API/worker; publisher and migrations use jobs. Application replicas can scale to zero; PostgreSQL and Test Cosmos provisioned throughput do not.
- ADRs 0001–0003 record foundation, Rust and managed ingress choices. Source: docs/architecture/2026-10-25-resource-center-handoff.md, as amended by these ADRs; old Python/Nginx choices are not current.
- ADR 0004 remains the selected direction. Loan phase 1 is locally implemented/tested; notification leases, secure browser flows and release changes remain planned. Current results and next work: `mem:backend/plan`.
- Deployment evidence and limits: `mem:cloud/plan`. Assessment gaps: `mem:assessment/requirements`.
