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
| **Service Bus Namespace** | `rc-test-0330980f-bus` (`notification-requests-v1`) | `rc-test-0330980f-platform-rg` | Basic tier queue (`max_delivery_count = 5`, `lock_duration = PT1M`, dead-lettering enabled) |
| **Cosmos DB for NoSQL** | `rc-test-0330980f-notifications` (`notifications`/`jobs`) | `rc-test-0330980f-platform-rg` | Notifications deduplication, delivery job state, and attempt audit records (`id = eventId`, `/eventId` partition key) |
| **Key Vault** | `rctest0330980fnkv` | `rc-test-0330980f-platform-rg` | Notifications secret store (`resend-api-key` when live email mode is enabled) |
| **Log Analytics Workspace** | `rc-test-0330980f-logs` | `rc-test-0330980f-platform-rg` | Central structured JSON logs, Container Apps system events, and saved KQL queries |
| **Loan API Container App** | `rc-test-0330980f-loan-api` | `rc-test-0330980f-workloads-rg` | Public HTTPS ingress (`0..3` replicas, HTTP scale concurrency `20`) |
| **Frontend Container App** | `rc-test-0330980f-web` | `rc-test-0330980f-workloads-rg` | Next.js standalone SSR portal (`0..3` replicas, HTTP scale concurrency `20`) |
| **Migrations Job** | `rc-test-0330980f-migrations` | `rc-test-0330980f-workloads-rg` | Manual trigger inside private VNet (`loan-service migrate`) |
| **Outbox Publisher Job** | `rc-test-0330980f-loan-publisher` | `rc-test-0330980f-workloads-rg` | Cron trigger (`*/1 * * * *`) claiming `outbox_events` and publishing to Service Bus |
| **Notifications Worker** | `rc-test-0330980f-notifications` | `rc-test-0330980f-workloads-rg` | Non-ingress worker (`0..3` replicas, KEDA `azure-servicebus` scale rule `messageCount = 5`) |
| **Alert Action Group** | `rc-test-0330980f-ops-ag` | `rc-test-0330980f-workloads-rg` | Routes metric alerts to Subscription `Owner` role and `G6605870@live.tees.ac.uk` |

---

## 2. Safe release, promotion and recovery

Use [the current backend release runbook](../backend/release-runbook.md). It replaces
the older apply-before-migrate path and typed promotion confirmation. Phase 4
implements and locally tests the release path; actual GitHub runs and gate settings
remain unverified. Only Test has earlier deployment evidence.

Do not roll back to pre-0002 writers after the due-date migration. Keep apps stopped
after failed migration until you inspect SQL state. Image rollback and SQL restore
are separate procedures; use the current runbook's compatibility checks.

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
