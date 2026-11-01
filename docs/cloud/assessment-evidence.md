# Resource Center — CIS3039 Cloud-Native DevOps Assessment Evidence Map

- **Date**: 2026-11-01
- **Assessment Specification Source**: `/home/cabozzz/Downloads/cnd-cis3039-assessment-spec.docx` (verified directly against original specification; never inferred from memory)
- **Live Cloud Environment**: Azure Test (`francecentral`, subscription `0330980f-85ea-4b38-8b9e-c871432c9d4f`)
- **Architecture Baselines**:
  - Original Handoff: [`docs/architecture/2026-10-25-resource-center-handoff.md`](file:///home/cabozzz/dev/resource.center/docs/architecture/2026-10-25-resource-center-handoff.md)
  - ADR 0001 (Azure Foundation & Free-Tier Selection): [`docs/architecture/adr/0001-azure-foundation.md`](file:///home/cabozzz/dev/resource.center/docs/architecture/adr/0001-azure-foundation.md)
  - ADR 0002 (Rust + Tokio + Axum Backend): [`docs/architecture/adr/0002-rust-backend.md`](file:///home/cabozzz/dev/resource.center/docs/architecture/adr/0002-rust-backend.md)
  - ADR 0004 (Loan rules and recovery): [backend design](../architecture/adr/0004-backend-use-cases-and-recovery.md)
  - ADR 0003 (Phase 2 Workloads & Managed Container Apps HTTPS Ingress): [`docs/architecture/adr/0003-phase2-workloads-and-ingress.md`](file:///home/cabozzz/dev/resource.center/docs/architecture/adr/0003-phase2-workloads-and-ingress.md)

---

## 1. Stage 1 — Architecture & C4 Diagrams

Topology reflects the selected design. Backend phase 1 is verified locally, not deployed by this change. Browser sign-in/actions remain phase 3 work. Stored Test results below are not a fresh live check.

### 1.1 C4 Level 1 — System Context Diagram

```mermaid
flowchart TB
    student["Student User\n(Authenticated via Auth0 OIDC/PKCE\nRole: student)"]
    staff["Staff User\n(Authenticated via Auth0 OIDC/PKCE\nRole: staff)"]
    anon["Unauthenticated Visitor\n(Campus Browser)"]

    rc["Campus Device Loan System\n(Resource Center)\nAzure Cloud-Native Platform"]

    auth0["Auth0 Identity Provider\n(External OAuth2 / OIDC + JWKS)"]
    resend["Resend Email API\n(External Transactional Email Provider)"]

    anon -->|"Browses device catalogue & availability (HTTPS)"| rc
    student -->|"Views availability, reserves available device (2-day loan),\nsubscribes to waitlist notifications (HTTPS + Bearer JWT)"| rc
    staff -->|"Views reservations, records physical collection\nand device return (HTTPS + Bearer JWT)"| rc

    rc -->|"Authenticates users (OIDC/PKCE) &\nvalidates RS256 Bearer JWTs via JWKS"| auth0
    rc -->|"Sends collection, return & availability\ntransactional emails (HTTPS API)"| resend
```

### 1.2 C4 Level 2 — Container Diagram

```mermaid
flowchart TB
    subgraph Users["Campus Users"]
        browser["Web Browser\n(Unauthenticated / Student / Staff)"]
    end

    subgraph External["External SaaS Providers"]
        auth0["Auth0\n(OAuth2/OIDC + JWKS)"]
        resend["Resend Email Service\n(Transactional Email API)"]
    end

    subgraph Azure["Azure Resource Center Platform (francecentral)"]
        web["Web Frontend Container App\n(Next.js 16.4.3 Standalone SSR)\nrc-test-0330980f-web"]

        subgraph LoanBoundary["Loan Service Boundary (Authoritative Business State)"]
            loan_api["Loan API Container App\n(Rust 1.94 / Tokio / Axum)\nrc-test-0330980f-loan-api"]
            migrations["Loan Migrations Job\n(Container Apps Job: loan-service migrate)\nrc-test-0330980f-migrations"]
            publisher["Loan Outbox Publisher Job\n(Scheduled Container Apps Job: */1 * * * *)\nrc-test-0330980f-loan-publisher"]
            pg[("Azure PostgreSQL Flexible Server\n(Private VNet 10.42.2.0/24, database: loan)\nTables: device_models, device_units,\nreservations, availability_subscriptions, outbox_events")]
        end

        sb[["Azure Service Bus Queue\n(Basic Tier: notification-requests-v1\n+ $DeadLetterQueue)"]]

        subgraph NotifBoundary["Notifications Service Boundary (Delivery & Deduplication State)"]
            notif_worker["Notifications Worker Container App\n(Rust 1.94 / Tokio + KEDA Queue Scaler)\nrc-test-0330980f-notifications"]
            cosmos[("Azure Cosmos DB for NoSQL\n(Database: notifications, Container: jobs\nPartition Key: /eventId, ETag concurrency)")]
            kv["Azure Key Vault\n(rctest0330980fnkv\nSecret: resend-api-key)"]
        end

        monitor["Azure Monitor & Log Analytics\n(rc-test-0330980f-logs + 6 Metric Alerts\n+ Action Group rc-test-0330980f-ops-ag)"]
    end

    browser -->|"HTTPS GET / (SSR Catalogue)"| web
    browser -->|"HTTPS REST /api/v1/*\n(Bearer JWT for protected reads/writes)"| loan_api
    web -->|"Server-side fetch /api/v1/catalogue"| loan_api
    loan_api -->|"Validates JWT signature, iss, aud, exp & role"| auth0

    migrations -->|"Applies SQL migrations & seeds models\n(Entra Managed Identity)"| pg
    loan_api -->|"Atomic ACID transactions:\nApplication use cases + domain rules; model-first locks; state + outbox_events"| pg
    publisher -->|"Claims unpublished outbox_events\n(FOR UPDATE SKIP LOCKED, 60s lease)"| pg
    publisher -->|"Publishes self-contained JSON event\n(MessageId = eventId, CorrelationId)"| sb

    sb -->|"Triggers KEDA scale-from-zero &\ndelivers PeekLock messages"| notif_worker
    notif_worker -->|"Upserts job state & ETag precondition\n(id = eventId)"| cosmos
    notif_worker -->|"Reads resend-api-key via RBAC"| kv
    notif_worker -->|"Delivers email (or FakeEmailSender in Test)"| resend

    loan_api -.->|"Structured JSON logs + metrics"| monitor
    publisher -.->|"Structured JSON logs + job metrics"| monitor
    notif_worker -.->|"Structured JSON logs + metrics"| monitor
```

### 1.3 Cloud Deployment & Network Isolation Diagram

```mermaid
flowchart LR
    internet(("Public Internet"))

    subgraph VNet["Virtual Network: rc-test-0330980f-vnet (10.42.0.0/16)"]
        subgraph CAE["Subnet: container-apps (10.42.0.0/23)"]
            web_app["rc-test-0330980f-web\n(External HTTPS Ingress :3000\nScale: 0..2 replicas)"]
            api_app["rc-test-0330980f-loan-api\n(External HTTPS Ingress :8080\nScale: 0..3 replicas)"]
            notif_app["rc-test-0330980f-notifications\n(No Ingress; KEDA Service Bus\nScale: 0..3 replicas)"]
            pub_job["rc-test-0330980f-loan-publisher\n(Cron Job: */1 * * * *)"]
            mig_job["rc-test-0330980f-migrations\n(Manual Job)"]
        end

        subgraph PGSubnet["Subnet: postgres (10.42.2.0/24 — Delegated)"]
            pg_srv[("rc-test-0330980f-loan-pg\n(Public Network Access: Disabled\nPrivate DNS: privatelink.postgres.database.azure.com)")]
        end
    end

    internet -->|"TLS 1.2/1.3 Managed Ingress"| web_app
    internet -->|"TLS 1.2/1.3 Managed Ingress"| api_app
    api_app -->|"Private VNet TCP :5432\n(Entra Token: rc-test-0330980f-loan)"| pg_srv
    pub_job -->|"Private VNet TCP :5432\n(Entra Token: rc-test-0330980f-publisher)"| pg_srv
    mig_job -->|"Private VNet TCP :5432\n(Entra Token: rc-test-0330980f-migrations)"| pg_srv
    servicebus["Azure Service Bus Basic / notification-requests-v1"]
    cosmos_jobs[("Cosmos DB for NoSQL / notifications / jobs")]
    keyvault["Key Vault / resend-api-key"]
    email["Resend HTTPS API; Test stored checks use fake email"]
    identity["Auth0 HTTPS JWKS; live setup pending"]
    pub_job -->|"HTTPS / managed identity sender"| servicebus
    servicebus -->|"HTTPS PeekLock / receiver identity / KEDA"| notif_app
    notif_app -->|"HTTPS / worker identity / ETag"| cosmos_jobs
    notif_app -->|"HTTPS / secret-reader identity"| keyvault
    notif_app -->|"HTTPS in live delivery mode"| email
    api_app -->|"HTTPS / signature verification"| identity
```

---

## 2. Stage 2 — Component Mapping, Security, Fakes, MVP & Build Priority

The [new Stage 2 annotated Container diagram](../backend/stage-2-plan.md) maps requirements, security, fakes, MVP scope and phase order. Tutor feedback/approval status is unknown. This table records supporting context.

| Priority Order | Container / Component | Functional & Non-Functional Requirements Mapped | Auth & Security Controls | Faked / Simulated Connections in Test | In MVP? |
| --- | --- | --- | --- | --- | --- |
| **1 (MVP Core)** | **PostgreSQL (`rc-test-0330980f-loan-pg`) + Migrations Job (`loan-service migrate`)** | Authoritative schema (`device_models`, `device_units`, `reservations`, `availability_subscriptions`, `outbox_events`), 2-day loan rule, transactional outbox durability, least-privilege publisher grants. | Private VNet subnet (`10.42.2.0/24`), public access disabled, Microsoft Entra Managed Identity tokens, `publisher` restricted to `outbox_events` (`SELECT`/`UPDATE`) and `notifications` identity denied (`28P01`). | None (real Azure PostgreSQL Flexible Server in Test). | **Yes (MVP DB 1)** |
| **2 (MVP Core)** | **Loan API (`rc-test-0330980f-loan-api`, `loan-service api`)** | Public device catalogue (`GET /api/v1/catalogue`), authenticated student reservation (`POST /api/v1/reservations`) with model-first row-lock concurrency control, waitlist subscription (`POST /api/v1/subscriptions`), staff collection (`POST /api/v1/reservations/{id}/collect`) & return (`POST /api/v1/reservations/{id}/return`), `/health/live` and `/health/ready`. | Managed TLS ingress, strict CORS allowlist (`ALLOWED_ORIGIN`), Auth0 RS256 JWKS validation (`Auth0JwtVerifier`), RBAC role enforcement (`student` vs `staff`), fails closed (`503 AuthNotConfigured`) when Auth0 env vars are blank. | In unit/integration tests: transactional fake storage and locally signed JWTs with cached test JWKS. In live Test: Auth0 is unconfigured (`auth0Configured: false`), so write routes fail closed (`503`) while live DB concurrency/RBAC are verified via `verify-resilience`. | **Yes (MVP Service 1)** |
| **3 (MVP Core)** | **Outbox Publisher Job (`rc-test-0330980f-loan-publisher`, `loan-service publish`) + Service Bus (`notification-requests-v1`)** | Asynchronous decoupling between Loan commits and Notifications; claims unpublished `outbox_events` rows with 60s lease (`claimed_until`), publishes self-contained event envelopes (`DeviceCollectionConfirmationRequested`, `DeviceReturnConfirmationRequested`, `DeviceAvailabilityNotificationRequested`) to Service Bus with `MessageId = eventId` and `CorrelationId`. | Passwordless Entra Managed Identity (`rc-test-0330980f-publisher`) with `Azure Service Bus Data Sender` role on `notification-requests-v1` only. | In unit tests: fake broker port. In live Test: real scheduled Container Apps Job + real Azure Service Bus Basic queue. | **Yes (MVP Async Bridge)** |
| **4 (MVP Core)** | **Notifications Worker (`rc-test-0330980f-notifications`) + Cosmos DB (`notifications`/`jobs`)** | KEDA scale-from-zero queue consumer; idempotency and delivery state tracking in Cosmos DB (`id = eventId`, `/eventId` partition key, `_etag` optimistic concurrency); retry classification (transient retry vs permanent dead-letter). | Passwordless Entra Managed Identity (`rc-test-0330980f-notifications`) with `Azure Service Bus Data Receiver`, Cosmos Built-in Data Contributor (`00000000-0000-0000-0000-000000000002`), and Key Vault Secrets User. Zero access to PostgreSQL. | **`FakeEmailSender` (`EMAIL_DELIVERY_MODE=fake`)**: Used in Test so zero real emails are dispatched while full Cosmos DB state transitions (`Claimed` → `Delivered`) and Service Bus settlement are exercised. (`ConfigurableEmailSender` supports Resend mode with a Key Vault secret). | **Yes (MVP Service 2 + MVP DB 2)** |
| **5 (Portal UI)** | **Web Frontend (`rc-test-0330980f-web`, Next.js 16.4.3)** | Responsive campus portal displaying real-time device models, categories, 2-day loan duration, available/total unit counts, architecture boundary status, and `/api/health` readiness endpoint. | Managed TLS ingress, non-root Node 22 Alpine container, server-side fetch to `LOAN_API_BASE_URL`. | Browser login and action flows are not implemented; status badges only show configuration. Credentials alone will not add those flows. | **Yes (Frontend Container)** |

---

## 3. Requirement evidence and limits

Source review: 2026-11-01. The rows below distinguish current code from stored runtime evidence. No live recheck ran during this review. Full findings: [cloud backend alignment](../assessment/2026-11-01-cloud-backend-alignment.md).

| Requirement | Status | Evidence and remaining work |
| --- | --- | --- |
| Public catalogue and counts | Implemented; stored Test evidence | API and Next.js catalogue exist. Confirm brand/model/category are clear in demo. Signed-in browser flows remain absent. |
| Reservation with return date | Implemented and verified locally; not deployed | Phase 1 assigns dueAt at reservation and preserves it on collection. Migration 0002 preserves valid old dates and backfills null dates. See [handoff](../backend/phase-1-handoff.md). |
| Concurrent reservation | Code plus stored PostgreSQL check | Model-first locks; phase 1 local PostgreSQL five-way race has one winner. Partial-write rollback, concurrent returns and multi-subscriber outbox checks also pass. HTTP tests use fake storage with real local JWT verification. |
| Availability subscription | Backend implemented | Route and SQL subscription exist. Browser action and live authenticated proof remain open. |
| Collection/return/availability messages | Backend plus stored fake-delivery checks | Return selects active subscribers for the model; collection/return state and outbox writes are atomic. Stored three-event flow reaches Cosmos with fake email, not real recipient delivery. |
| Auth0/JWT/RBAC | API code; incomplete user flow | JWT validator and role checks exist. Recorded Test configuration returns AuthNotConfigured on protected routes. Browser login/actions and live valid/invalid-token/403 evidence remain open. |
| Resilience/idempotency | Partial evidence | Stored publisher lease recovery and tests for terminal duplicate/retry handling. Concurrent nonterminal worker claims, send/crash windows and full outage/backlog recovery still need proof. |
| Observability | Code and stored Test checks | JSON logs, correlation, probes, publisher metrics, KQL and six alerts exist. Synthetic alert-routing test was blocked by Azure Students policy; actual delivery not verified. |
| CI/CD and promotion | Workflow files implemented; execution proof incomplete | YAML defines Test automation and manual promotion confirmation. Need successful GitHub runs and verified environment gate settings. Test rollout precedes migrations; promotion lacks migration wait/smoke checks. |
| IaC/scaling | Code and stored Test evidence | Terraform roots and remote state exist; worker scale-from-zero recorded. Forty successful catalogue requests are limited load evidence, not proof of HTTP scale-out. Staging/Production only parameterized. |

## 4. Remaining work

- Obtain tutor approval for the prepared Stage 2 diagram, MVP/fakes/build order and due-date rule.
- Deploy the tested due-date migration/API through a safe release; implement browser sign-in/student/staff actions.
- Configure and verify Auth0 and email where required by approved scope; do not label fake outputs as live results.
- Complete concurrent duplicate/failure recovery tests and capture real GitHub pipeline/gate evidence.
- Make release order and promotion checks safe; reconcile disabled Staging/Production seed settings with per-deployment demo-data requirement.
- Produce the 1,500–2,000-word report, screenshots and clips, then the required Media/Source/Documentation ZIP and root readme.
