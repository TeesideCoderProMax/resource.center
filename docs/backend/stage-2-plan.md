# Stage 2 annotated Container plan

Date: 2026-11-01. Design follows ADRs 0002–0004. Backend phase 1 is implemented and tested locally. Other phases remain planned. **Tutor feedback and approval status: unknown; no record was supplied. This assessment note does not block backend work.** MVP labels below are the proposed scope for tutor review.

This new diagram copies the current Container structure and adds requirement, security, fake and build-order labels. The two business services are Loan and Notifications; the publisher belongs to Loan.

```mermaid
flowchart TB
    visitor["Visitor / student / staff"]
    auth["MVP dependency: Auth0\nOIDC + PKCE; RS256 JWKS\nPhase 3: live sign-in proof pending"]
    subgraph System["Campus Device Loan System"]
        web["MVP UI: Next.js\nR1 catalogue; R2 reserve/subscribe; R3 staff flows\nManaged HTTPS; no client secrets\nCatalogue exists; action screens in phase 3"]
        subgraph Loan["MVP service 1: Loan"]
            api["Rust + Tokio + Axum API\nR1 catalogue/counts; R2 due dates/subscriptions\nR3 collection/return timestamps; R4 atomic allocation/outbox\nJWT issuer/audience/expiry + student/staff RBAC\nPhase 1: application use cases + domain rules"]
            pg[("MVP dependency: private PostgreSQL\nLoan state + outbox; model-first row locks\nTLS + per-process managed identity\nPhase 1: migration 0002 + transaction tests")]
            pub["MVP process: Loan publisher\nR5 async delivery; recoverable outbox lease\nOutbox-only DB grants; Service Bus sender\nExisting process; phase 4 release checks"]
            mig["MVP process: migration job\nSchema-owner identity; separate seed SQL\nPhase 1 schema; phase 4 safe rollout"]
        end
        queue["MVP dependency: Service Bus Basic\nR5 durable async JSON v1 / PeekLock\nEntra sender/receiver roles; dead-letter queue"]
        subgraph Notifications["MVP service 2: Notifications"]
            worker["Rust + Tokio worker; no public ingress\nR5 confirmations + availability email\nReceiver identity; no PostgreSQL access\nPhase 2: owned leases/retry/recovery"]
            cosmos[("MVP dependency: Cosmos DB for NoSQL\nJobs/attempts; eventId partition + ETag\nWorker identity only; phase 2 claim tests")]
        end
        logs["MVP operations: logs / health / metrics\nCorrelation IDs; alerts\nPhase 4: CI gates and scaling evidence"]
    end
    resend["MVP email dependency: Resend\nHTTPS; API key from Key Vault\nPhase 3: approved live recipient proof"]
    fakeStore["Local fake storage\nSame Loan use cases; copy-on-success\nPhase 1 unit/HTTP tests"]
    fakeAuth["Local JWT fixture + cached test JWKS\nReal RS256 verification; no live Auth0\nTests only; phase 1 route contracts"]
    fakeDelivery["Fake broker / job store / email ports\nLocal tests; stored Test uses fake email\nNo proof of received email"]
    visitor -->|"HTTPS"| web
    web -.->|"Phase 3 browser OIDC/PKCE"| auth
    visitor -->|"HTTPS JSON; Bearer JWT for protected routes"| api
    web -->|"HTTPS public catalogue fetch"| api
    api -->|"JWKS HTTPS; verify token and role"| auth
    api -->|"TLS SQL; state + outbox in one transaction"| pg
    mig -->|"TLS SQL; migrate before matching API"| pg
    pub -->|"TLS SQL; claim committed outbox"| pg
    pub -->|"HTTPS REST; eventId + correlationId"| queue
    queue -->|"HTTPS PeekLock; KEDA scale signal"| worker
    worker -->|"HTTPS; conditional ETag writes"| cosmos
    worker -->|"HTTPS; format and send"| resend
    api -.->|"Test substitute for storage only"| fakeStore
    api -.->|"Test identity provider data"| fakeAuth
    pub -.-> fakeDelivery
    worker -.-> fakeDelivery
    api -.-> logs
    pub -.-> logs
    worker -.-> logs
```

| Label | Requirement and evidence target |
| --- | --- |
| R1 | Public brand/model/category catalogue and unit counts. Existing name carries brand/model. |
| R2 | Signed-in reservation with automatic two-day due date and model subscription. Phase 1 API; browser flow phase 3. |
| R3 | Staff collection/return, role checks and timestamps. Local signed-JWT routes tested; live proof phase 3. |
| R4 | One winner for last unit; state/outbox rollback together. Real local PostgreSQL tests in phase 1. |
| R5 | Collection/return confirmations and all active subscribers on return. Stable v1 requests in phase 1; worker recovery phase 2; received email phase 3. |
| Operations | Stateless processes, bounded DB connections, durable queue, health/logs/metrics, IaC, Test CI and gated promotion. Existing source/stored evidence; phase 4 closes evidence gaps. |

Build order: (1) Loan rules, ports, migration and tests; (2) notification lease/retry/recovery; (3) Auth0 plus student/staff screens and email proof; (4) release order, seeding, CI/gates and scaling evidence. Screens and owned worker leases are locally implemented (phases 2–3). Phase 4 locally tests the release path; live acceptance and tutor approval remain open. See [phase 4 handoff](phase-4-handoff.md).

The brief asks for tutor approval of final MVP choices. The review can cover scope, the reservation-based 48-hour clock and fakes in the demo. Local implementation proceeds under the user's request; it does not grant tutor approval.

Current C4/deployment diagrams: [assessment evidence map](../cloud/assessment-evidence.md). API and event details: [contracts](contracts.md). No cross-service database access or synchronous Loan lookup by Notifications is allowed.
