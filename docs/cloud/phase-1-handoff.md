# Phase 1 handoff

Updated 2026-10-30. Status: **APPLIED — foundation checks verified** in France
Central, Test. Business workloads, application migrations and email delivery
remain phase 2 work. No deployment commit was created.

## Deployment and ownership

Subscription `0330980f-85ea-4b38-8b9e-c871432c9d4f` (Azure for Students), tenant
`43d2115b-a55e-46b6-9df7-b03388ecfc60`. Azure's subscription API confirms
`spendingLimit=On`. User reports USD 100 student credit; remaining credit and
first-year PostgreSQL/registry benefit expiry were not independently verified.

Terraform applied 8 bootstrap and 43 platform resources. Required resource
providers were registered. Resource groups:

- `rc-test-0330980f-state-rg`: Terraform backend.
- `rc-test-0330980f-platform-rg`: network, data services, identities and environment.
- `rc-test-0330980f-workloads-rg`: reserved for phase 2; disposable checks cleaned up.
- Azure manages an additional `ME_...` infrastructure group for Container Apps.

Existing lab4-devops, InSintonia and NetworkWatcherRG resources were not modified.
France Central is permitted by the subscription's region policy. The managed
Container Apps environment contains only its Consumption profile; no business
application containers or schedules exist yet. Scale-to-zero settings belong to
phase 2; provisioned PostgreSQL and free-tier Cosmos capacity remain allocated.

## State and interfaces

Storage account `rctest0330980fstate`, keys disabled, TLS 1.2 minimum, private
containers, versioning, and 30-day blob/container recovery. Bootstrap state was
migrated from local state into the backend. Containers / keys:

| Container | Key | Access |
|---|---|---|
| bootstrap | test/bootstrap.tfstate | privileged operator |
| platform | test/platform.tfstate | privileged operator |
| workloads | test/workloads.tfstate | workload deployer and operator |

Roots are `infra/bootstrap`, `infra/platform`, and phase 2's reserved
`infra/workloads`. Provider AzureRM 4.81.0 and dependency lockfiles are present.
Use explicit `ENV=test` and `ROOT=bootstrap|platform` Makefile commands documented
in `infra/README.md`. Backend/input examples are nonsensitive; actual local
configuration, state, plans and exports are ignored. Preserve Azure-selected
PostgreSQL zone `3` and the PostgreSQL subnet's Microsoft.Storage endpoint;
these are now declared in configuration. Final plans for both roots exit 0 with
**no changes**. No environment was destroyed/replaced to remove drift.

[Exported foundation interface](test-foundation-outputs.json) contains selected
nonsensitive resource IDs, identity IDs, endpoints and deployment parameters.
Phase 2 consumes this explicit interface, never raw platform/bootstrap state.

## Data services and permissions

PostgreSQL 16, B1ms, 32 GiB, zone 3, seven-day backup, no geo-redundancy. Public
access disabled; delegated subnet/private DNS; Entra-only authentication with
password authentication disabled. Human Entra administrator retained. Service
principals initialized through a temporary private-network administrator job;
temporary elevated access is removed. Authentication tokens require refresh and
TLS verification in application adapters.

| Identity | Verified/prepared access |
|---|---|
| Loan | ACR pull; PostgreSQL Loan-schema DML; no DDL, Cosmos or notification-vault access |
| Publisher | ACR pull; PostgreSQL login/schema usage; queue Send, denied Receive/Cosmos/vault; outbox-table rights await phase 2 migration |
| Notifications | ACR pull; Cosmos jobs access; queue Receive, denied Send; notification-vault lookup allowed; PostgreSQL authentication denied |
| Migrations | ACR pull; PostgreSQL Loan-schema ownership/DDL |
| GitHub workloads | Contributor on workload RG; Reader on platform RG; attach prepared identities/environment; push images; own state container only |

The GitHub deployer is trusted to run code as these attached identities. OIDC
subject: `repo:TeesideCoderProMax/resource.center:environment:test`. Git origin
matches, but gh repository/environment API checks returned not-found. Actual
GitHub access and branch/environment protection controls still need verification
before enabling workflows. No CI workflow was deployed here.

Cosmos NoSQL account: **free tier enabled**, local keys disabled; manual 400 RU/s
shared database throughput, account ceiling 1,000 RU/s. Database `notifications`,
container `jobs`, partition `/eventId`; stable single-recipient eventId is also
the job id. Attempts share the partition; use conditional ETag updates and create
conflicts for duplicate handling. No default TTL until retention/privacy/replay
policy is established. One subscription free-tier slot is now used by this account.

Service Bus Basic: queue `notification-requests-v1`, 14-day TTL, ten deliveries,
one-minute lock, expiration dead-lettering; no duplicate-detection/session claim.
At-least-once handling, renewal, retries and DLQ operations remain application /
phase 3 responsibilities. Notification vault requires future secret
`resend-api-key`; no real secret value was created or read. Auth0/Resend connections
remain phase 2. No email was sent by diagnostics.

## Live verification — 2026-10-30

Disposable checks used an immutable diagnostic image from the project's ACR:
`foundation-check@sha256:40b14986711094b93ab63a9b39544c5746ef2285752cf2682cdebd88073e9768`.
Source and rerun instructions are in `infra/diagnostics`. Manual jobs use 0.25
vCPU / 0.5 GiB, bounded runtime, no recurring schedule.

- PASS: bootstrap remote-state migration and real blob-lease contention; a second
  Terraform plan was denied while a competing lease was held, then lease released.
- PASS: disposable blob restored from a prior version, downloaded contents checked,
  sentinel deleted. This exercises backend blob recovery, not a PostgreSQL restore.
- PASS: private PostgreSQL DNS/TLS and managed-identity logins; migration DDL and
  Loan insert/read/update/delete; Loan/publisher denied schema/database creation.
- PASS: Loan/publisher denied Cosmos and notification-vault access.
- PASS: publisher sends; receive denied. Notifications receives/settles that
  synthetic message; send denied. Synthetic request contains no recipient/email.
- PASS: Notifications creates/reads/deletes Cosmos job, duplicate create conflicts,
  stale ETag update rejected; PostgreSQL authentication denied.
- PASS: Notifications vault lookup is authorized and returns not-found for an
  intentionally absent test secret; this does not verify a real Resend secret.
- PASS: ACR identity pulls, demonstrated by successful diagnostic executions.
- Terraform formatting, validation and final no-change bootstrap/platform plans
  passed. Shell syntax, Python compilation and memory/reference checks also ran.

State-isolation and cleanup results are recorded in the completion record below.

## Remaining phase 2 / operations work

No business schema, outbox tables, app containers, frontend, API ingress, Auth0
configuration, Resend credentials or real email integration exists yet. Managed
HTTPS replacing Nginx/Let's Encrypt remains prospective; preserve the accepted
historical baseline until adopted. No Next.js sources exist to decide export/SSR.

PostgreSQL/Cosmos backup settings are configured, but database restore drills have
not run. Cross-environment isolation is structurally parameterized; only Test is
deployed. Do not assume Staging/Production can claim the same Cosmos free slot.
No cost-alert threshold or recipient was supplied; budget alerts remain absent.
Student first-year allowances can expire independently of remaining credit; see
[test costs](test-costs.md). Phase 3 owns monitoring, alerting and gated workflows.

### Completion record

All seven diagnostic executions reached terminal **Succeeded**; execution IDs are
in [the check record](test-foundation-checks.json). The workload deployer verified
read/write/delete in its own state container and received 403 for both bootstrap
and platform state. The cleanup execution removed the disposable PostgreSQL
table. Its live replica log was already reclaimed when fetched; terminal success
was verified through the execution API. Other service PASS logs were inspected.

All seven diagnostic jobs were deleted, followed by the dedicated bootstrap
identity and its ACR Pull assignment. The only retained PostgreSQL Entra admin is
the configured human administrator. No scheduled/running diagnostic compute is
left. The small immutable diagnostic image remains in ACR for reproducibility.
A cleanup approval review initially timed out before executing; the bounded
cleanup was subsequently executed successfully. No user approval is outstanding.

Final limitations: no real Resend secret/email flow, business concurrency/outbox
suite, GitHub environment protections, database restore drills or cost alerts
were verified. These are explicit follow-on work, not claimed phase 1 results.

Final Azure inventory returned an empty workload resource group after cleanup;
queue active, dead-letter, scheduled and transfer message counts were all zero.

## Backend stack revision — 2026-10-30

The accepted backend is now Rust + Tokio + Axum: Axum/Tokio Loan API,
Rust/Tokio publisher and independent Notifications worker. See
`docs/architecture/adr/0002-rust-backend.md`. The deployed phase 1 resources
and exported interface require no change; phase 2 owns Rust images and adapter
integration tests. Existing Python diagnostics remain operator tooling.
