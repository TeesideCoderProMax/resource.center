# Stack
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Next.js frontend; frontend JavaScript versus TypeScript and versions/package manager not specified. Serena typescript backend supports both.
- Python/FastAPI/asyncio Loan API, private PostgreSQL; independent Python Notifications worker with private Azure Cosmos DB for NoSQL; Resend delivery adapter.
- Nginx HTTPS ingress, Let's Encrypt certificates; private HTTP/JSON forwarding is design intent pending network/TLS review.
- Auth0 OIDC + PKCE frontend sign-in; JWT API validation and student/staff authorization.
- Durable queue accepted; Azure Service Bus proposed default pending availability/budget/hosting/SKU decisions.
- No application dependencies or executable commands installed/selected. Python and typescript Serena languages configured in anticipation of sources.
