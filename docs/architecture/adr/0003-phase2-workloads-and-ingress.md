# ADR 0003: Phase 2 serverless workloads, ingress and frontend hosting

Date: 2026-10-31. Status: **prepared for Phase 2; adoption of managed HTTPS
ingress and Container Apps frontend hosting recorded**. Preserves the original
2026-10-25 handoff (`docs/architecture/2026-10-25-resource-center-handoff.md`)
as historical evidence and builds on ADR 0001 and ADR 0002.

## Context and findings

1. **Managed HTTPS ingress vs Nginx/Let's Encrypt**:
   The phase 1 foundation (`rc-test-0330980f-apps`) runs Azure Container Apps
   Consumption, which provides managed TLS termination, HTTP-to-HTTPS redirect,
   revision traffic routing and HTTP concurrency scaling out of the box. Running
   a separate self-managed Nginx + Let's Encrypt container inside Container Apps
   would duplicate TLS termination and prevent direct platform HTTP scaling to
   zero on the Loan API.
2. **Next.js frontend hosting in France Central**:
   Azure Static Web Apps (`Microsoft.Web/staticSites`) is only available in
   `centralus`, `eastus2`, `westus2`, `westeurope` and `eastasia`. Live Azure
   policy inspection on subscription `0330980f-85ea-4b38-8b9e-c871432c9d4f`
   (`sys.regionrestriction`) permits only `francecentral`, `belgiumcentral`,
   `spaincentral`, `polandcentral` and `germanywestcentral`, and `Microsoft.Web`
   is `NotRegistered`. Therefore Static Web Apps cannot be provisioned in this
   subscription. Deploying the Next.js frontend as a separate Container App in
   the existing France Central Consumption environment (`min_replicas = 0`)
   satisfies the subscription policy, uses the existing subscription-wide
   Container Apps free allowance, and avoids registering `Microsoft.Web`.
3. **Service Bus queue-depth scaling authentication**:
   Container Apps KEDA `azure-servicebus` custom scale rules support managed
   identity authentication (`identity_id` set to the Notifications user-assigned
   managed identity resource ID, `namespace` set to the Service Bus namespace
   name, `queueName = "notification-requests-v1"`). The Notifications identity
   already holds `Azure Service Bus Data Receiver` on the queue (`Microsoft.ServiceBus/*/queues/read`
   and `Microsoft.ServiceBus/*/receive/action`), allowing KEDA to read queue
   runtime message counts and wake the worker from zero replicas without SAS
   connection strings.
4. **Database and Cosmos boundaries**:
   - PostgreSQL Flexible Server (`rc-test-0330980f-loan-pg`) remains managed
     provisioned B1ms compute (not a scale-to-zero serverless database).
   - Versioned SQL migrations run via a dedicated manual Container Apps Job
     (`rc-test-0330980f-migrations`) using only the `migrations` identity, which
     owns schema `loan` and grants `SELECT, UPDATE` on `loan.outbox_events` to
     the `publisher` principal. Routine migrations are separated from deterministic
     Test demo seeding (`SEED_DEMO_DATA=true`) and never perform destructive resets.
   - Cosmos DB (`notifications`/`jobs`, partition `/eventId`, `id = eventId`)
     stores notification jobs with explicit `status` (`Claimed`, `Delivered`,
     `RetryableFailure`, `TerminalFailure`), `attemptCount`, `deliveryMode`,
     `recipientRedacted` and co-partitioned `attempts` history, updated via
     optimistic concurrency (`If-Match` / `_etag`) and 409 Conflict deduplication.

## Decisions

- Adopt Azure Container Apps managed HTTPS ingress for the public Next.js
  frontend (`rc-<env>-<suffix>-web`) and Rust/Tokio/Axum Loan API
  (`rc-<env>-<suffix>-loan-api`), superseding self-managed Nginx/Let's Encrypt
  while preserving `docs/architecture/2026-10-25-resource-center-handoff.md` as
  historical baseline evidence.
- Deploy the Next.js frontend as a standalone Container App (`min_replicas = 0`,
  `max_replicas = 3`) in `rc-test-0330980f-workloads-rg`, configured with runtime
  API/Auth0 environment variables and no privileged credentials in client bundles.
- Deploy the Loan API as an Axum/Tokio Container App (`min_replicas = 0`,
  `max_replicas = 3`, bounded DB pool size `max_connections = 5` per replica),
  with `/health/startup`, `/health/live` and `/health/ready` probes, CORS locked
  to the frontend origin, and Auth0 JWT validation scaffold (`AUTH0_ISSUER`,
  `AUTH0_AUDIENCE`, JWKS validation when configured; explicit `503 AuthNotConfigured`
  on protected routes when Auth0 tenant settings are not yet supplied).
- Deploy the Loan outbox publisher as a finite scheduled Container Apps Job
  (`rc-<env>-<suffix>-loan-publisher`, `cron_expression = "*/1 * * * *"`, bounded
  batch size) that claims committed `loan.outbox_events` rows using
  `FOR UPDATE SKIP LOCKED`, publishes to Service Bus, and marks `PUBLISHED` only
  after broker `201 Created` acknowledgement.
- Deploy the Notifications worker as a no-ingress Rust/Tokio Container App
  (`rc-<env>-<suffix>-notifications`, `min_replicas = 0`, `max_replicas = 3`)
  scaled by the KEDA `azure-servicebus` rule with managed identity authentication,
  using a safe `FakeEmailSender` adapter by default (`EMAIL_DELIVERY_MODE=fake`)
  unless `EMAIL_DELIVERY_MODE=resend` and `RESEND_AUTHORIZED_RECIPIENTS` are
  explicitly configured alongside Key Vault secret `resend-api-key`.
