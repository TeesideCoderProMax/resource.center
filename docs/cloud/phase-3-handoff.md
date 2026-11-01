# Phase 3 Handoff — Automated Delivery, Operations & Assessment Evidence

- **Date**: 2026-11-01
- **Status**: **Deployed & Verified in Azure Test (`francecentral`); Parameterized & Validated for Staging and Production**
- **Subscription**: `Azure for Students` (`0330980f-85ea-4b38-8b9e-c871432c9d4f`), Tenant `43d2115b-a55e-46b6-9df7-b03388ecfc60`
- **Machine-Readable Evidence**:
  - Workloads Outputs: [`docs/cloud/test-workloads-outputs.json`](file:///home/cabozzz/dev/resource.center/docs/cloud/test-workloads-outputs.json)
  - Phase 3 Checks: [`docs/cloud/test-phase3-checks.json`](file:///home/cabozzz/dev/resource.center/docs/cloud/test-phase3-checks.json)
  - Operational Runbook: [`docs/cloud/runbook.md`](file:///home/cabozzz/dev/resource.center/docs/cloud/runbook.md)
  - Assessment Evidence Map: [`docs/cloud/assessment-evidence.md`](file:///home/cabozzz/dev/resource.center/docs/cloud/assessment-evidence.md)

---

## 1. Summary of Phase 3 Deliverables

### 1.1 CI/CD & Gated Promotion Pipelines
- **[`.github/workflows/ci.yml`](file:///home/cabozzz/dev/resource.center/.github/workflows/ci.yml)**:
  - Runs on `pull_request` and `push` to `main` with `permissions: contents: read` (zero cloud secrets or OIDC tokens exposed to untrusted/fork PRs).
  - Executes `make backend-fmt`, `make backend-lint` (`cargo clippy --all-targets -- -D warnings`), `make backend-test` (10 unit/integration tests across `rc-shared`, `loan-service`, and `notifications-worker`), `make frontend-typecheck`, `make frontend-build`, `make frontend-audit`, `make infra-fmt`, `make infra-validate ENV=test|staging|production`, and `make docker-build`.
- **[`.github/workflows/deploy.yml`](file:///home/cabozzz/dev/resource.center/.github/workflows/deploy.yml)**:
  - **Per-Environment Serialization**: Uses `concurrency: group: deploy-${{ inputs.target_environment || 'test' }}` with `cancel-in-progress: false` so Terraform remote state locks never collide.
  - **Automated Merge-to-Test (`deploy-test` job)**:
    1. Blocked on `verify` job passing.
    2. Authenticates to Azure via Federated OIDC (`azure/login@v2`, `rc-test-0330980f-github-deploy`, client ID `b8dba55f-9eb5-4d3f-9f5d-cfe913b62104`).
    3. Builds and pushes `rc-backend` and `rc-web` to `rctest0330980facr.azurecr.io`, resolving immutable `@sha256:...` digests.
    4. Updates `infra/environments/test/workloads.tfvars` and runs `make infra-plan ENV=test ROOT=workloads && make infra-apply ENV=test ROOT=workloads` so **Terraform remains the sole owner of image references** (preventing CLI drift).
    5. Runs `rc-test-0330980f-migrations` job inside the private VNet and blocks until `Succeeded`.
    6. Runs `make smoke-test` ([`infra/scripts/smoke-test.sh`](file:///home/cabozzz/dev/resource.center/infra/scripts/smoke-test.sh)) verifying `/health/live`, `/health/ready` (including exact `service_version` match), `/api/v1/catalogue`, unauthenticated write rejection (`401`/`503`), and frontend `/api/health`.
  - **Gated Promotion to Staging & Production (`promote` job)**:
    - Triggered via `workflow_dispatch` with `target_environment = staging | production`.
    - Enforces **dual approval gates**:
      1. GitHub Environment protection (`environment: staging` / `environment: production`).
      2. Explicit workflow confirmation token (`approval_confirmation == PROMOTE-STAGING` or `PROMOTE-PRODUCTION`) and regex validation that `backend_image_digest` and `web_image_digest` are immutable `@sha256:...` references (promoting the exact tested images without rebuilding).
    - *Note on GitHub CLI inspection*: The repository remote `git@github.com:TeesideCoderProMax/resource.center.git` uses a repo-local SSH deploy/account key (`.git/config`), whereas the local `gh` CLI is logged into a separate account (`CodeZobac`), returning 404 on private repo settings queries. Implementing both `environment:` and the explicit `PROMOTE-<ENV>` input gate guarantees promotion gating regardless of GitHub account tier.

### 1.2 Environment Parameterization (`test`, `staging`, `production`)
- Isolated backend and variable templates created under:
  - [`infra/environments/staging/`](file:///home/cabozzz/dev/resource.center/infra/environments/staging/platform.tfvars.example) (`bootstrap.tfvars.example`, `platform.backend.hcl.example`, `platform.tfvars.example`, `workloads.backend.hcl.example`, `workloads.tfvars.example`)
  - [`infra/environments/production/`](file:///home/cabozzz/dev/resource.center/infra/environments/production/platform.tfvars.example) (`bootstrap.tfvars.example`, `platform.backend.hcl.example`, `platform.tfvars.example`, `workloads.backend.hcl.example`, `workloads.tfvars.example`)
- Both `staging` and `production` set `cosmos_capacity_mode = "serverless"` (since `test` occupies the subscription's single Cosmos free-tier slot) and `seed_demo_data = false`. All 9 root/environment combinations (`bootstrap`, `platform`, `workloads` × `test`, `staging`, `production`) pass `make infra-validate`.

### 1.3 Observability, Saved KQL Searches & Metric Alerts (Provisioned in Azure Test)
- **Log Analytics Saved Searches ([`infra/platform/monitoring.tf`](file:///home/cabozzz/dev/resource.center/infra/platform/monitoring.tf), applied to `rc-test-0330980f-logs`)**:
  - `rc-correlation-trace`: End-to-end trace across `loan-api`, `loan-publisher`, and `notifications-worker` by `correlationId` / `eventId`.
  - `rc-outbox-publisher-metrics`: Extracts `scanned`, `published`, and `elapsedMs` per publisher job run.
  - `rc-service-warnings-and-errors`: Surfaces all `WARN` and `ERROR` structured log events.
- **Action Group & Metric Alerts ([`infra/workloads/monitoring.tf`](file:///home/cabozzz/dev/resource.center/infra/workloads/monitoring.tf), applied to `rc-test-0330980f-workloads-rg`)**:
  - Action Group `rc-test-0330980f-ops-ag` (`Enabled`): Routes alerts with Common Alert Schema to ARM Role `Owner` (`8e3af657-a8ff-443c-a75c-2fe8c4bcb635`) and operator email `G6605870@live.tees.ac.uk`.
  - 6 Metric Alerts (`enabled = true`):
    1. `rc-test-0330980f-loan-api-5xx-alert` (`Requests` with `statusCodeCategory = 5xx > 0`, Sev 2)
    2. `rc-test-0330980f-servicebus-dlq-alert` (`DeadletteredMessages > 0` on `notification-requests-v1`, Sev 1)
    3. `rc-test-0330980f-servicebus-backlog-alert` (`ActiveMessages > 25` on `notification-requests-v1`, Sev 2)
    4. `rc-test-0330980f-job-failures-alert` (`CompletedExecutions` with `status = Failed > 0` on `rc-test-0330980f-loan-publisher`, Sev 2)
    5. `rc-test-0330980f-postgres-health-alert` (`is_db_alive < 1` on `rc-test-0330980f-loan-pg`, Sev 1)
    6. `rc-test-0330980f-postgres-pressure-alert` (`cpu_percent > 80` or `storage_percent > 85` on `rc-test-0330980f-loan-pg`, Sev 2)

---

## 2. Verified Release, Rollback, Resilience & Scaling Evidence

1. **Phase 3 Image Rollout & Previous-Image Rollback**:
   - Built and pushed `rctest0330980facr.azurecr.io/rc-backend@sha256:28ae9ffebf54b256c0ebe12a03700ae6a204c1763b702b5763278a3f311ab09a` (`0.1.0-phase3`).
   - Rolled back `infra/workloads` to the Phase 2 digest (`@sha256:3e0e9dbafe7026695e90c696512da9f05666b4f25bc811f6ead5ee2700a0510a`, revision `rc-test-0330980f-loan-api--0000004`) and verified `/health/ready` reported `"version":"0.1.0-phase2"`.
   - Rolled forward back to the Phase 3 digest (`@sha256:28ae9ffebf54b256c0ebe12a03700ae6a204c1763b702b5763278a3f311ab09a`, revision `rc-test-0330980f-loan-api--0000005`) and verified `/health/ready` reported `"version":"0.1.0-phase3"`.
2. **Live PostgreSQL Concurrency, Crash-Window Lease Recovery & 3 Notification Flows (`rc-test-0330980f-migrations-9111ssy`)**:
   - Executed `loan-service verify-resilience` inside the private VNet against `rc-test-0330980f-loan-pg`:
     - **5-way concurrent reservation race on 1 unit**: `SELECT ... FOR UPDATE SKIP LOCKED` produced exactly `1` winner and `4` `NoAvailableUnits` conflicts, with `due_at - reserved_at == 172800s` (2 days).
     - **RBAC & Rollback Atomicity**: Staff reservation attempt rejected with `ForbiddenRole("staff")` and `0` outbox rows written.
     - **Publisher Crash-Window Recovery**: Simulated an expired 60-second publisher lease (`claimed_until = NOW() - 120s`) on `evt-b731901c-798f-5306-b615-d13717c64bf2`; `rc-test-0330980f-loan-publisher` reclaimed and published all 3 events (`evt-b731901c-798f-5306-b615-d13717c64bf2` `DeviceCollectionConfirmationRequested`, `evt-b4ea50c9-04fe-5637-9cb6-18808ab406e1` `DeviceReturnConfirmationRequested`, `evt-eb69eb66-832b-5a7f-8a52-2390a806aa2a` `DeviceAvailabilityNotificationRequested`) to Service Bus `notification-requests-v1`.
     - **End-to-End KEDA & Cosmos DB Delivery**: `rc-test-0330980f-notifications` consumed all 3 events, recorded `"final_status":"Delivered"` in Cosmos DB (`notifications`/`jobs`) via `FakeEmailSender` (0 real emails sent), and settled the queue (`activeMessageCount: 0`, `deadLetterMessageCount: 0`).
3. **Concurrent HTTP Load Burst**:
   - Executed 40 requests (`concurrency = 20`) against `https://rc-test-0330980f-loan-api.livelywave-2cc0564d.francecentral.azurecontainerapps.io/api/v1/catalogue`: `40/40` returned `HTTP 200` (`0` errors).

---

## 3. Remaining External / Student-Owned Items

- **Live Auth0 Tenant & Resend API Key**: `auth0Configured` remains `false` (write endpoints fail closed with `503 AuthNotConfigured`) and `EMAIL_DELIVERY_MODE` remains `fake` until external credentials are provided.
- **Tutor Stage 2 Feedback & Final Written Report / Demo Media**: Documented in [`docs/cloud/assessment-evidence.md`](file:///home/cabozzz/dev/resource.center/docs/cloud/assessment-evidence.md) for student submission.
