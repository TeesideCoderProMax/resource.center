# ADR 0002: Rust backend with Tokio and Axum

Date: 2026-10-30. Status: **accepted by user; application implementation pending**.
Source: user instruction, “The backend stack will be Rust + Tokio + Axum”.
This supersedes the Python/FastAPI/asyncio backend selection in the 2026-10-25
handoff. That document and historical sources remain preserved as evidence.

## Selected stack and boundaries

Use Rust for the Loan API, Loan-owned outbox publisher and independent
Notifications worker. Tokio provides the async runtime. Axum provides HTTP
routing/middleware for the Loan API and any explicitly required HTTP probes;
the queue worker and scheduled publisher do not need an HTTP server for their
ordinary work. Next.js, Auth0, PostgreSQL, Cosmos DB for NoSQL and Resend remain
part of the design.

Keep domain/application logic independent of Axum, Tokio-specific runtime
mechanisms and vendor SDKs where possible. HTTP handlers and queue consumers
translate inputs into use cases; infrastructure implements ports and composition
roots supply adapters. Preserve PostgreSQL ownership, atomic outbox writes,
partition-aware Cosmos deduplication and at-least-once delivery. Do not assume
exactly-once email delivery or cross-service transactions.

## Infrastructure impact

The deployed Test Container Apps Consumption environment runs language-independent
Linux containers. No Terraform resource, state, database, queue, identity, region,
free-tier selection or permission needs changing for this language decision.
No Azure apply or resource replacement is performed by this change.

Phase 2 will build Rust binaries in reproducible multi-stage containers, with
non-root runtime images, trusted TLS roots, compatible native dependencies and
explicit shutdown handling. Pin Rust/toolchain and crate versions when actual
Cargo manifests are introduced; retain Cargo.lock for deployed applications.
Choose PostgreSQL/migration, Azure identity, Cosmos, Service Bus, JWT and Resend
adapters after verifying their current Rust compatibility and integration tests.
The existing Python diagnostics prove platform permissions/connectivity, not
that any Rust adapter already works.

Keep the proposed HTTP/queue minimum replicas at zero and bound maximum replicas
and database pools. Scale-to-zero applies to app containers, not PostgreSQL or
provisioned Cosmos free-tier capacity. Managed HTTPS replacing Nginx/Let's Encrypt
remains the separate prospective decision described in ADR 0001.

## Implementation status

No Rust application, Cargo workspace, container image or workflow exists yet.
This change updates the accepted stack, phase plans and Serena language setup.
Python operator scripts under infra/diagnostics and JSON checking in the Terraform
wrapper remain tooling; migrating them is not required for a Rust backend.
No claim of assessment compliance follows from this stack decision.

References checked:
- [Container Apps supports any runtime/language in supported Linux images](https://learn.microsoft.com/en-us/azure/container-apps/containers).
- [Axum compatibility with Tokio](https://docs.rs/axum/latest/axum/#compatibility).
