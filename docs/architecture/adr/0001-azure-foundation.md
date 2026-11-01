# ADR 0001: Azure foundation for Test

Date: 2026-10-29; updated 2026-10-30. Status: foundation applied to Test
following user deployment authorization. Live foundation checks are recorded in
`docs/cloud/phase-1-handoff.md`. This ADR does not change the historical accepted
application ingress handoff.

## Decisions encoded in Terraform

Use a Container Apps workload-profile environment containing only Consumption,
with separate identities for Loan, publisher, Notifications and migrations.
Use provisioned PostgreSQL Flexible Server 16 (B1ms, 32 GiB, seven-day backup)
over a delegated private subnet and private DNS. PostgreSQL is not serverless.
Use Entra authentication only. SQL role creation is an explicit private-network
bootstrap step; Azure management RBAC does not create database privileges.

Following the user’s 2026-10-29 free-tier-first instruction, Notifications uses a
single-region Cosmos NoSQL free-tier account with shared manual 400 RU/s
(database scope), an account throughput ceiling of 1,000 RU/s, and local
keys disabled. `jobs` partitions on `/eventId`; one request event addresses one
recipient, and its stable eventId is also the job's `id`. Repeated delivery uses
exactly the same pair. Attempts may be separate items in the same partition,
allowing transactional batches with that job. Claim/update with ETags; create
conflicts mean a job already exists. No default expiry: automatic dedupe expiry
would make old replay unsafe. Phase 2 must define PII erasure and tombstone
retention before real recipient data, with no exactly-once email claim.

Service Bus Basic meets the single-queue need without an hourly Standard base
charge. No broker duplicate detection or sessions are assumed. Retention is
14 days, peek lock one minute, ten deliveries before dead-lettering; expiration
also dead-letters. The worker must renew locks, handle duplicate requests, and
settle only after durable handling. Phase 3 owns DLQ monitoring/replay procedures.

Use ACR Standard to match the eligible first-12-month student allowance, with
admin credentials disabled; Key Vault RBAC with purge
protection for notification secrets; Log Analytics with 30-day retention and a
1 GiB/day ingestion quota (a loss-of-observability tradeoff, not a cost cap).
PostgreSQL is network-private. Cosmos, queue, vault, registry and state endpoints
remain public TLS endpoints with identity authorization; this is not network
isolation. No blanket private endpoints or costly premium SKUs are assumed.

Separate bootstrap, platform and future workload state containers; environment
uses an independent storage account/resource groups. The workload deployer has
only its state container, not platform/bootstrap state. Platform changes remain
an operator action requiring deliberate role-assignment rights. Workload deployer
can attach identities and therefore is trusted with their effective privileges.
OIDC subject is repository + exact GitHub environment; branch protections and
approval controls must be checked before enabling workflows.

## Proposed hosting change, not yet adopted

Managed Azure HTTPS ingress would replace Nginx/Let's Encrypt in phase 2. The
existing accepted baseline and AGENTS.md continue to apply until this change is
explicitly adopted. The foundation creates no ingress or application containers.
No Next.js sources exist to establish export compatibility. Prefer Static Web
Apps static export; confirm against application requirements in phase 2, using a
separate Container App if SSR is needed. Do not silently use hybrid preview.

## Sources checked

- [Container Apps consumption and billing](https://learn.microsoft.com/en-us/azure/container-apps/billing)
- [Terraform Azure backend locking and Entra authentication](https://developer.hashicorp.com/terraform/language/backend/azurerm)
- [PostgreSQL managed identity setup](https://learn.microsoft.com/en-us/azure/postgresql/security/security-connect-with-managed-identity)
- [Cosmos data-plane RBAC](https://learn.microsoft.com/en-us/azure/cosmos-db/how-to-connect-role-based-access-control)
- [Service Bus TTL](https://learn.microsoft.com/en-us/azure/service-bus-messaging/message-expiration)
- AzureRM 4.81.0 installed schema, pinned with signed provider lockfiles.

The only existing Cosmos account was verified as serverless with free tier disabled;
the subscription slot was available at inspection. Serverless remains an explicit
Terraform option, not the default. Confirm PostgreSQL/ACR offer expiry before apply;
student credit balance alone does not prove free-service eligibility.
