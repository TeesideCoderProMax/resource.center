# Phase 2 Cloud Workloads & Edge Handoff

Date: 2026-10-31  
Environment: `test`  
Region: `francecentral`  
Subscription: `0330980f-85ea-4b38-8b9e-c871432c9d4f`  
Tenant: `43d2115b-a55e-46b6-9df7-b03388ecfc60`

This handoff records the completed Phase 2 deployment of the Campus Device Loan workloads in Azure, live verification evidence, and the exact inputs required for Phase 3 (CI/CD, observability alerts, and security/reliability runbooks).

---

## 1. Deployed Workloads Summary

All Phase 2 compute resources are provisioned via the isolated Terraform root `infra/workloads` (remote state in `rctest0330980fstate`, container `workloads`, key `test/workloads.tfstate`) into resource group `rc-test-0330980f-workloads-rg`:

| Workload | Azure Resource Name | Type / Trigger | Managed Identity | Scaling / Ingress | Image Digest |
| --- | --- | --- | --- | --- | --- |
| Loan HTTP API | `rc-test-0330980f-loan-api` | Container App (Consumption) | `rc-test-0330980f-loan` | `min_replicas = 0`, `max_replicas = 3` (HTTP concurrency `20`); external managed HTTPS ingress on port `8080` | `rctest0330980facr.azurecr.io/rc-backend@sha256:3e0e9dbafe7026695e90c696512da9f05666b4f25bc811f6ead5ee2700a0510a` |
| Next.js Web Frontend | `rc-test-0330980f-web` | Container App (Consumption) | `rc-test-0330980f-loan` (ACR pull only) | `min_replicas = 0`, `max_replicas = 3` (HTTP concurrency `20`); external managed HTTPS ingress on port `3000` | `rctest0330980facr.azurecr.io/rc-web@sha256:c01bb79aff68d6268812083430f6091d37614f65a764e8b17c51b8d4bc9fdee0` |
| Loan Schema Migrations | `rc-test-0330980f-migrations` | Container Apps Job (`Manual` trigger) | `rc-test-0330980f-migrations` | Bounded single-shot job (`timeout = 180s`, `retry_limit = 0`), command `["loan-service", "migrate"]` | `rctest0330980facr.azurecr.io/rc-backend@sha256:3e0e9dbafe7026695e90c696512da9f05666b4f25bc811f6ead5ee2700a0510a` |
| Loan Outbox Publisher | `rc-test-0330980f-loan-publisher` | Container Apps Job (`Schedule` trigger `*/1 * * * *`) | `rc-test-0330980f-publisher` | Bounded batch job (`timeout = 60s`, `retry_limit = 1`), command `["loan-service", "publish"]` | `rctest0330980facr.azurecr.io/rc-backend@sha256:3e0e9dbafe7026695e90c696512da9f05666b4f25bc811f6ead5ee2700a0510a` |
| Notifications Worker | `rc-test-0330980f-notifications` | Container App (Consumption) | `rc-test-0330980f-notifications` | `min_replicas = 0`, `max_replicas = 3`; **no HTTP ingress**; KEDA `azure-servicebus` custom scale rule on `notification-requests-v1` | `rctest0330980facr.azurecr.io/rc-backend@sha256:3e0e9dbafe7026695e90c696512da9f05666b4f25bc811f6ead5ee2700a0510a` |

### Public Endpoints (Test)

- **Web Frontend URL**: `https://rc-test-0330980f-web.livelywave-2cc0564d.francecentral.azurecontainerapps.io`
- **Loan API Base URL**: `https://rc-test-0330980f-loan-api.livelywave-2cc0564d.francecentral.azurecontainerapps.io`
  - Startup / Liveness / Readiness probes: `/health/startup`, `/health/live`, `/health/ready`
  - Public Catalogue: `GET /api/v1/catalogue`
  - Protected Routes (Auth0 JWT): `POST /api/v1/reservations`, `POST /api/v1/subscriptions`, `POST /api/v1/reservations/{id}/collect`, `POST /api/v1/reservations/{id}/return`

---

## 2. Key Architectural Decisions Established in Phase 2

Recorded in [ADR 0003](file:///home/cabozzz/dev/resource.center/docs/architecture/adr/0003-phase2-workloads-and-ingress.md):

1. **Managed Container Apps HTTPS Ingress (Superseding VM Self-Managed Nginx)**:
   - Azure Container Apps managed Envoy ingress terminates TLS (`allow_insecure_connections = false`) on both `rc-test-0330980f-web` and `rc-test-0330980f-loan-api`.
   - `rc-test-0330980f-notifications` has no ingress block and cannot be reached over HTTP.
2. **Next.js Frontend Hosted on Azure Container Apps in France Central**:
   - `Microsoft.Web/staticSites` (Azure Static Web Apps) is only offered in `Central US`, `East US 2`, `West US 2`, `West Europe`, and `East Asia`.
   - Subscription policy `sys.regionrestriction` (`Allowed resource deployment regions`) strictly permits only `francecentral`, `belgiumcentral`, `spaincentral`, `polandcentral`, and `germanywestcentral`.
   - Therefore, Next.js (`16.4.3`, `output: "standalone"`) runs as a scale-to-zero Container App (`rc-test-0330980f-web`) in `francecentral`.
3. **Passwordless Runtime Data-Plane Authentication**:
   - **PostgreSQL**: `loan-service` acquires Entra tokens for `https://ossrdbms-aad.database.windows.net` via the Container Apps `IDENTITY_ENDPOINT` using its user-assigned identity (`loan`, `migrations`, or `publisher`) and connects over TLS with a bounded concurrency semaphore (`PG_MAX_CONNECTIONS = 5`).
   - **Service Bus**: `loan-service publish` sends messages via HTTPS REST (`POST /notification-requests-v1/messages`) with `https://servicebus.azure.net` Bearer tokens; `notifications-worker` receives messages via peek-lock (`POST /notification-requests-v1/messages/head`), renews locks in the background, and settles (`DELETE`) or abandons (`PUT`) locks based on durable Cosmos outcomes.
   - **Cosmos DB**: `notifications-worker` authenticates to the Cosmos DB SQL REST API using Entra RBAC tokens (`https://cosmos.azure.com`, `type=aad&ver=1.0&sig=...`) on database `notifications`, container `jobs` (partition key `/eventId`, `id = eventId`) with `If-Match` optimistic concurrency.

---

## 3. Live Verification Evidence (`docs/cloud/test-workloads-checks.json`)

1. **Private VNet Migrations & Least-Privilege Grant**:
   - Job execution `rc-test-0330980f-migrations-r8veoqm` succeeded inside the private Container Apps Environment subnet, applying `0001_initial_loan_schema.sql`, seeding the 4 deterministic device models (`dell-latitude-7440`, `macbook-air-m3`, `ipad-air-m2`, `sony-alpha-a6400`), and granting `SELECT, UPDATE ON loan.outbox_events` to `rc-test-0330980f-publisher`.
2. **HTTPS Ingress, Readiness, Catalogue, and Auth Rejection**:
   - `GET /health/ready` returned `200 OK` (`"status": "ready"`, `"database": "reachable"`, `"version": "0.1.0-phase2"`).
   - `GET /api/v1/catalogue` returned `200 OK` with all 4 seeded device models and echoed `x-correlation-id`.
   - `POST /api/v1/reservations` without Auth0 configuration returned `503 Service Unavailable` (`"error": "AuthNotConfigured"`) and `access-control-allow-origin: https://rc-test-0330980f-web.livelywave-2cc0564d.francecentral.azurecontainerapps.io`.
   - `GET https://rc-test-0330980f-web.../api/health` returned `200 OK`, and `GET /` server-rendered the 4 device models from the Loan API.
3. **End-to-End Outbox -> Publisher -> Service Bus -> KEDA Wake-from-Zero -> Cosmos DB**:
   - Synthetic probe `evt-phase2-e2e-probe-001` (`correlation_id = corr-phase2-e2e-probe-001`, `syntheticProbe = true`) was inserted into `loan.outbox_events`.
   - Scheduled job `rc-test-0330980f-loan-publisher` claimed the row with `FOR UPDATE SKIP LOCKED`, published it to `notification-requests-v1`, and marked the outbox row `PUBLISHED`.
   - KEDA scaled `rc-test-0330980f-notifications` from `0` to `1` replica (`KEDAScaleTargetActivated`, triggered by `s0-azure-servicebus-notification-requests-v1`).
   - `notifications-worker` claimed the job in Cosmos DB (`notifications`/`jobs`), executed `FakeEmailSender` (zero live emails sent), updated the Cosmos document to `Delivered` with `If-Match` ETag precondition (erasing raw `recipientDelivery` and retaining `recipientRedacted: "p***@example.invalid"`), and completed the Service Bus message (`activeMessageCount: 0, deadLetterMessageCount: 0`).
4. **Service Boundary Isolation Proof (`notifications-worker verify-isolation`)**:
   - Executed inside the live `rc-test-0330980f-notifications` replica:
     - Key Vault lookup (`resend-api-key`): `404_not_found_rbac_allowed`
     - Cosmos DB create + `409 Conflict` + ETag replace + `412 Precondition Failed` on stale ETag + delete cleanup: `passed_and_cleaned`
     - PostgreSQL connection attempt with `rc-test-0330980f-notifications` identity: rejected with `FATAL: password authentication failed for user "rc-test-0330980f-notifications"`.
5. **Terraform No-Drift Verification**:
   - `make infra-plan ENV=test ROOT=bootstrap`, `make infra-plan ENV=test ROOT=platform`, and `make infra-plan ENV=test ROOT=workloads` all report `No changes. Your infrastructure matches the configuration.`

---

## 4. Pending External Credentials & Phase 3 Inputs

- **Auth0 OIDC Credentials**: `auth0_domain`, `auth0_audience`, and `auth0_client_id` are wired as Terraform variables and container environment variables (`AUTH0_ISSUER`/`AUTH0_DOMAIN`, `AUTH0_AUDIENCE`, `NEXT_PUBLIC_AUTH0_*`), currently set to `""` until the external Auth0 tenant is configured.
- **Resend API Key**: `EMAIL_DELIVERY_MODE` defaults to `"fake"`. To enable live Resend delivery for allowlisted test recipients in Phase 3, populate secret `resend-api-key` in Key Vault `https://rctest0330980fnkv.vault.azure.net/` and set `EMAIL_DELIVERY_MODE=resend` + `RESEND_AUTHORIZED_RECIPIENTS`.
- **Phase 3 CI/CD Identity**: User-assigned identity `rc-test-0330980f-github-workloads` (`client_id = b8dba55f-9eb5-4d3f-9f5d-cfe913b62104`) is ready for GitHub Actions OIDC image push, `infra/workloads` apply, and `rc-test-0330980f-migrations` job execution.
