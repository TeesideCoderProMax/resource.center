# Decisions
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Accepted: Resource Center/Campus Device Loan; Next.js; Nginx/Let's Encrypt; Python/FastAPI/asyncio/PostgreSQL Loan; Auth0; independent Python Notifications with Cosmos DB for NoSQL/Resend; durable queue; loan-owned transactional outbox/publisher; clean layers.
- Superseded: shared notification database alternative, earlier uncertainty about product/stack. Prior notes retained at `docs/architecture/history/initial-serena-notes/`.
- Proposed: Azure Service Bus default, Cosmos serverless evaluation, v1 request contract. Not provisioned or fully configured.
- Pending: hosting/regions/budget/IaC/network, broker SKU/retry/retention policy, Cosmos partition/dedup/conditional claim design, provider idempotency, dependency versions/tooling. No canonical ADRs yet; handoff is current accepted design source.
- Verified: repository setup and documentation only; application/assessment compliance unverified.
- Proposed first complete slice: authorized staff return → PostgreSQL state plus outbox commit → acknowledged publication → queue consumer → Cosmos job/outcome and Resend adapter. Include invalid-state/authorization checks, rollback, duplicates/crash recovery and outage/backlog recovery; see `mem:architecture/testing`.
