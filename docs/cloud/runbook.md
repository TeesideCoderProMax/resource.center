# Resource Center — Cloud Operational Runbook

- **Date**: 2026-11-01
- **Environment Baseline**: Azure Test (`francecentral`, subscription `0330980f-85ea-4b38-8b9e-c871432c9d4f`, tenant `43d2115b-a55e-46b6-9df7-b03388ecfc60`)
- **Parameterized Environments**: Staging (`rc-staging-0330980f-*`) and Production (`rc-production-0330980f-*`) under `infra/environments/{staging,production}/*.example`
- **Architecture References**: `docs/architecture/2026-10-25-resource-center-handoff.md`, `docs/architecture/adr/0001-azure-foundation.md`, `docs/architecture/adr/0002-rust-backend.md`, `docs/architecture/adr/0003-phase2-workloads-and-ingress.md`

---

## 1. Service & Infrastructure Inventory (Test)

| Component | Azure Resource Name | Resource Group | Role & Ownership |
| --- | --- | --- | --- |
| **Remote State Storage** | `rctest0330980ftfstate` (`tfstate` container) | `rc-test-0330980f-bootstrap-rg` | Holds `bootstrap.tfstate`, `platform.tfstate`, `workloads.tfstate` |
| **GitHub OIDC Identity** | `rc-test-0330980f-github-deploy` | `rc-test-0330980f-bootstrap-rg` | Passwordless federated credential (`repo:TeesideCoderProMax/resource.center:*`) |
| **Container Registry** | `rctest0330980facr.azurecr.io` | `rc-test-0330980f-platform-rg` | Standard SKU (`admin_enabled = false`); stores immutable `rc-backend` and `rc-web` digests |
| **PostgreSQL Flexible Server** | `rc-test-0330980f-loan-pg` (`db: loan`) | `rc-test-0330980f-platform-rg` | Authoritative Loan state + `outbox_events`; private VNet-delegated (`10.42.2.0/24`), Entra auth enabled |
| **Service Bus Namespace** | `rc-test-0330980f-bus` (`notification-requests-v1`) | `rc-test-0330980f-platform-rg` | Standard tier queue (`max_delivery_count = 5`, `lock_duration = PT1M`, dead-lettering enabled) |
| **Cosmos DB for NoSQL** | `rc-test-0330980f-notifications` (`notifications`/`jobs`) | `rc-test-0330980f-platform-rg` | Notifications deduplication, delivery job state, and attempt audit records (`id = eventId`, `/eventId` partition key) |
| **Key Vault** | `rctest0330980fnkv` | `rc-test-0330980f-platform-rg` | Notifications secret store (`resend-api-key` when live email mode is enabled) |
| **Log Analytics Workspace** | `rc-test-0330980f-logs` | `rc-test-0330980f-platform-rg` | Central structured JSON logs, Container Apps system events, and saved KQL queries |
| **Loan API Container App** | `rc-test-0330980f-loan-api` | `rc-test-0330980f-workloads-rg` | Public HTTPS ingress (`0..3` replicas, HTTP scale concurrency `20`) |
| **Frontend Container App** | `rc-test-0330980f-web` | `rc-test-0330980f-workloads-rg` | Next.js standalone SSR portal (`0..2` replicas, HTTP scale concurrency `30`) |
| **Migrations Job** | `rc-test-0330980f-migrations` | `rc-test-0330980f-workloads-rg` | Manual trigger inside private VNet (`loan-service migrate`) |
| **Outbox Publisher Job** | `rc-test-0330980f-loan-publisher` | `rc-test-0330980f-workloads-rg` | Cron trigger (`*/1 * * * *`) claiming `outbox_events` and publishing to Service Bus |
| **Notifications Worker** | `rc-test-0330980f-notifications` | `rc-test-0330980f-workloads-rg` | Non-ingress worker (`0..3` replicas, KEDA `azure-servicebus` scale rule `messageCount = 5`) |
| **Alert Action Group** | `rc-test-0330980f-ops-ag` | `rc-test-0330980f-workloads-rg` | Routes metric alerts to Subscription `Owner` role and `G6605870@live.tees.ac.uk` |

---

## 2. Standard Deployment & Gated Promotion

### 2.1 Local Developer & CI Verification
Before pushing or deploying, run the unified check suite from the repository root:

```bash
make check
```

This runs:
- `make backend-fmt` (`cargo fmt --all --check`)
- `make backend-lint` (`cargo clippy --all-targets -- -D warnings`)
- `make backend-test` (`cargo test` across `rc-shared`, `loan-service`, and `notifications-worker`)
- `make frontend-typecheck`, `make frontend-build`, `make frontend-audit`
- `make infra-fmt` and `make infra-validate ENV=test|staging|production`

### 2.2 Automated Merge-to-Test Pipeline (`.github/workflows/deploy.yml`)
Every push to `main` runs the `verify` job first. Only if all checks pass does `deploy-test` execute:
1. Authenticates to Azure using federated OIDC (`azure/login@v2` with `rc-test-0330980f-github-deploy`).
2. Builds `rc-backend` and `rc-web`, pushes them to `rctest0330980facr.azurecr.io`, and resolves their immutable `@sha256:...` digests.
3. Injects the exact `@sha256:...` digests into `infra/environments/test/workloads.tfvars` so **Terraform remains the sole owner of workload image references** (preventing out-of-band CLI drift).
4. Runs `make infra-init ENV=test ROOT=workloads`, `make infra-plan ENV=test ROOT=workloads`, and `make infra-apply ENV=test ROOT=workloads`.
5. Starts `rc-test-0330980f-migrations` and polls execution status until `Succeeded` (failing the workflow if migrations fail).
6. Runs `make smoke-test` (`infra/scripts/smoke-test.sh`) verifying `/health/live`, `/health/ready` (including exact `service_version` match and `"database":"reachable"`), `/api/v1/catalogue`, unauthenticated write rejection (`401`/`503`), and frontend `/api/health`.

### 2.3 Gated Promotion to Staging & Production
Staging and Production never rebuild container images; they promote the exact `@sha256:...` digests verified in Test.
1. Trigger `.github/workflows/deploy.yml` via `workflow_dispatch`.
2. Select `target_environment` (`staging` or `production`).
3. Provide the verified `backend_image_digest` (`...@sha256:...`) and `web_image_digest` (`...@sha256:...`).
4. Set `approval_confirmation` to `PROMOTE-STAGING` or `PROMOTE-PRODUCTION`.
5. Both the GitHub Environment protection gate (`environment: staging` or `environment: production`) and the explicit confirmation token must pass before Terraform plans or applies `infra/workloads`.

---

## 3. Rollback & Database Recovery Procedures

### 3.1 Application Image Rollback (Terraform-Owned)
Because Terraform owns `backend_image`, `web_image`, and `service_version`, rollbacks are performed by pinning the previous verified `@sha256:...` digest in `workloads.tfvars` and applying `infra/workloads`:

```bash
# 1. Pin previous verified digest and version in workloads.tfvars
sed -i 's|^backend_image[[:space:]]*=.*|backend_image            = "rctest0330980facr.azurecr.io/rc-backend@sha256:3e0e9dbafe7026695e90c696512da9f05666b4f25bc811f6ead5ee2700a0510a"|' infra/environments/test/workloads.tfvars
sed -i 's|^service_version[[:space:]]*=.*|service_version          = "0.1.0-phase2"|' infra/environments/test/workloads.tfvars

# 2. Plan and apply
make infra-plan ENV=test ROOT=workloads
make infra-apply ENV=test ROOT=workloads

# 3. Verify rollback via smoke test
EXPECTED_VERSION="0.1.0-phase2" make smoke-test
```

### 3.2 Database Migration & Point-in-Time Restore Policy
- **Container rollback does not undo database schema changes.**
- **Expand/Contract Rule**: All SQL migrations in `backend/crates/loan-service/migrations/` must be additive/backward-compatible (add nullable columns, new tables, or indexes first) so that the N-1 container image continues to operate safely if rolled back.
- **Failed Migration Recovery**:
  - Each migration runs inside a PostgreSQL transaction (`BEGIN ... COMMIT` recorded in `schema_migrations`). If a DDL statement fails mid-flight, PostgreSQL rolls back that migration file automatically and `rc-test-0330980f-migrations` exits non-zero, blocking deployment.
- **Catastrophic Data Corruption / Point-in-Time Restore (PITR)**:
  - `rc-test-0330980f-loan-pg` is configured with `backup_retention_days = 7` (Staging: `7`, Production: `14`).
  - To restore PostgreSQL to a timestamp prior to data corruption:
    ```bash
    az postgres flexible-server restore \
      --resource-group rc-test-0330980f-platform-rg \
      --name rc-test-0330980f-loan-pg-restored \
      --source-server rc-test-0330980f-loan-pg \
      --restore-time "2026-11-01T11:30:00Z" \
      --vnet "/subscriptions/0330980f-85ea-4b38-8b9e-c871432c9d4f/resourceGroups/rc-test-0330980f-platform-rg/providers/Microsoft.Network/virtualNetworks/rc-test-0330980f-vnet" \
      --subnet "postgres" \
      --private-dns-zone "/subscriptions/0330980f-85ea-4b38-8b9e-c871432c9d4f/resourceGroups/rc-test-0330980f-platform-rg/providers/Microsoft.Network/privateDnsZones/privatelink.postgres.database.azure.com"
    ```

---

## 4. Diagnosing Incidents, Outbox Backlog & Dead-Letter Queue (DLQ)

### 4.1 Saved Log Analytics Queries
Three saved KQL searches are provisioned in `rc-test-0330980f-logs` (`infra/platform/monitoring.tf`):
1. **`rc-correlation-trace`**: Traces a single `correlationId` or `eventId` across `loan-api`, `loan-publisher`, and `notifications-worker` with PII redacted.
2. **`rc-outbox-publisher-metrics`**: Extracts `scanned`, `published`, and `elapsedMs` from each `rc-test-0330980f-loan-publisher` execution.
3. **`rc-service-warnings-and-errors`**: Lists all `WARN`/`ERROR` log lines across all containers and jobs.

Run a live correlation trace from the CLI:
```bash
WORKSPACE_ID="$(az monitor log-analytics workspace show -g rc-test-0330980f-platform-rg -n rc-test-0330980f-logs --query customerId -o tsv)"
az monitor log-analytics query \
  --workspace "${WORKSPACE_ID}" \
  --analytics-query "ContainerAppConsoleLogs_CL | where Log_s has 'corr-' or Log_s has 'evt-' | project TimeGenerated, ContainerAppName_s, Log_s | order by TimeGenerated desc | take 25" \
  -o table
```

### 4.2 Recovering Outbox Backlog
If Service Bus experiences a transient outage or `rc-test-0330980f-loan-publisher` fails:
1. Loan transactions (`POST /api/v1/reservations`, `/collect`, `/return`) **continue to commit normally** in PostgreSQL along with their `outbox_events` rows (`published_at IS NULL`).
2. Rows claimed by a crashed publisher replica automatically become eligible again after their 60-second lease expires (`claimed_until < NOW()`).
3. To trigger an immediate outbox drain without waiting for the 1-minute cron schedule:
   ```bash
   az containerapp job start \
     --resource-group rc-test-0330980f-workloads-rg \
     --name rc-test-0330980f-loan-publisher
   ```

### 4.3 Inspecting & Safely Reprocessing Dead-Letter Messages
When a message fails delivery `max_delivery_count = 5` times (or fails contract validation with `PermanentContractError`), Service Bus moves it to `notification-requests-v1/$DeadLetterQueue` and triggers `rc-test-0330980f-servicebus-dlq-alert` (Severity 1).

1. **Check active and dead-letter counts**:
   ```bash
   az servicebus queue show \
     --resource-group rc-test-0330980f-platform-rg \
     --namespace-name rc-test-0330980f-bus \
     --name notification-requests-v1 \
     --query countDetails -o json
   ```
2. **Diagnose why the message dead-lettered**:
   Query `ContainerAppConsoleLogs_CL` for `notifications-worker` errors and check the Cosmos DB `notifications`/`jobs` item (`id = <eventId>`), which records `status` (`Failed` or `DeadLettered`), `attempts`, and `lastError`.
3. **Safe Reprocessing**:
   - Because `notifications-worker` enforces idempotency in Cosmos DB (`id = eventId` with `_etag` optimistic concurrency), re-enqueuing or re-publishing an event from `outbox_events` is safe:
     - Already-`Delivered` events short-circuit (`AlreadyDeliveredSkipped`) without sending duplicate emails.
     - Transiently failed events increment `attempts` and retry delivery.
   - If the root cause was a downstream outage (e.g., Resend API outage after live credentials are enabled), fix the downstream secret/service first, then reset `published_at = NULL, claimed_until = NULL` on the affected `event_id` in PostgreSQL `outbox_events` (via a controlled admin job) or move the message from `$DeadLetterQueue` back to `notification-requests-v1`.

---

## 5. Credential Rotation

- **Azure Service-to-Service Auth**: Uses **User-Assigned Managed Identities** (`rc-test-0330980f-loan`, `rc-test-0330980f-migrations`, `rc-test-0330980f-publisher`, `rc-test-0330980f-notifications`) and short-lived Entra ID OAuth2 tokens (`https://ossrdbms-aad.database.windows.net/.default`, `https://servicebus.azure.net/.default`, `https://cosmos.azure.com/.default`, `https://vault.azure.net/.default`). There are no connection strings or database passwords stored in application configuration.
- **Resend API Key Rotation**:
  1. Write the new secret version to Key Vault `rctest0330980fnkv`:
     ```bash
     az keyvault secret set --vault-name rctest0330980fnkv --name resend-api-key --value "<NEW_RESEND_KEY>"
     ```
  2. Restart or roll `rc-test-0330980f-notifications` so new worker instances fetch the latest secret version.
- **PostgreSQL Bootstrap Administrator Password**:
  - Used only by Terraform/migrations bootstrap if needed; rotate via `az postgres flexible-server update -g rc-test-0330980f-platform-rg -n rc-test-0330980f-loan-pg --admin-password "<NEW_STRONG_PASSWORD>"` and update local gitignored `platform.tfvars`.

---

## 6. Cost Profile & Safe Teardown

### 6.1 Remaining Fixed Costs (Scale-to-Zero != Zero Total Cost)
While all Container Apps (`loan-api`, `web`, `notifications`) scale to **zero replicas when idle** and `rc-test-0330980f-notifications` Cosmos DB uses the **provisioned free-tier discount** in Test, the following resources incur continuous non-zero metered cost while provisioned:
1. **Azure Database for PostgreSQL Flexible Server (`rc-test-0330980f-loan-pg`, `B_Standard_B1ms`, 32 GB storage)**: Managed provisioned compute (not serverless scale-to-zero). Can be **stopped** via `az postgres flexible-server stop -g rc-test-0330980f-platform-rg -n rc-test-0330980f-loan-pg` between demo/marking sessions so only storage is billed.
2. **Azure Service Bus Standard (`rc-test-0330980f-bus`)**: Base hourly/monthly namespace charge required for KEDA queue-depth scaling and dead-letter queues.
3. **Azure Container Registry Standard (`rctest0330980facr`)**: Base daily registry fee (~$0.167/day).
4. **Azure Monitor Metric Alerts & Log Analytics (`rc-test-0330980f-logs`)**: Free up to the first 5 GB/month of log ingestion; metric rules have a small per-rule monthly evaluation charge (~$0.10/rule/month).

### 6.2 Safe Teardown Procedure (Preserving Evidence & State)
To tear down the live Test workloads and platform after assessment marking while preserving all repository evidence (`docs/cloud/*.json`, `docs/cloud/*.md`) and remote state history:

```bash
# 1. Destroy workloads first (Container Apps, Jobs, Metric Alerts, Action Group)
./infra/scripts/terraform.sh destroy workloads test

# 2. Destroy platform (PostgreSQL, Cosmos DB, Service Bus, ACR, Key Vault, VNet)
./infra/scripts/terraform.sh destroy platform test

# 3. Optional: Destroy bootstrap only when remote state is no longer needed
./infra/scripts/terraform.sh destroy bootstrap test
```

> **Guardrail**: Never touch unrelated subscription resource groups (`lab4-devops`, `InSintonia`, `NetworkWatcherRG`).
