# Selected baseline
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Next.js → Nginx → Python/FastAPI/asyncio Loan API → PostgreSQL.
- PostgreSQL transactional outbox → loan-owned polling publisher → durable queue → independent Python Notifications worker → Cosmos DB / Resend.
- Publisher may run separately but remains within Loan boundary, with access to its outbox. Notifications has no PostgreSQL credentials; Loan API has no Cosmos access.
- Auth0: OIDC + PKCE sign-in, JWT signature/issuer/audience/expiry validation using provider signing keys, server-side student/staff authorization.
- Nginx/Let's Encrypt HTTPS ingress selected; private HTTP forwarding subject to actual hosting/TLS requirements.
- Pending: hosting/runtime/regions/IaC, broker SKU/configuration, Cosmos capacity/partitioning, network requirements. Assess Cosmos serverless; do not assume Azure Functions.
