# Azure foundation

Test foundation deployed and checked on 2026-10-30; read
`docs/cloud/phase-1-handoff.md` before changes or applying another environment.
Terraform >=1.9,<2 and AzureRM **4.81.0** are required. Use Azure CLI login locally,
or OIDC environment variables in CI. Never put tokens/passwords in tfvars or
backend files. Lockfiles are committed; state, plans and local overrides are not.

## Ownership and isolation

| Root | Owner | State |
|---|---|---|
| bootstrap | privileged operator | bootstrap container, ENV/bootstrap.tfstate |
| platform | privileged operator | platform container, ENV/platform.tfstate |
| workloads (phase 2) | environment-scoped GitHub identity | workloads container, ENV/workloads.tfstate |

Each environment has its own storage account and resource groups. A workload
identity gets no Terraform state access. GitHub deployment has blob data rights
only on `workloads`; no storage account keys are enabled. Export only
`terraform output -json handoff` for phase 2, not raw state. Do not use Terraform
workspaces. Keep bootstrap local state encrypted and backed up until migration.

## Review and initialize

1. Confirm subscription, region, cost, globally unique names and existing resource
   ownership. Examples use discovered subscription IDs, not secrets. Copy
   `infra/environments/test/*.example` to their suffix-free names. For another
   environment create a separate directory, change **all** environment/names/state
   scopes, and choose appropriate production capacity/HA rather than assuming
   the Test defaults are production-ready.
2. The operator needs resource creation rights and role-assignment rights at the
   new resource groups (subscription Owner can bootstrap, but is not granted to
   CI). Register required providers deliberately: Microsoft.Storage, Network,
   App, OperationalInsights, ContainerRegistry, KeyVault, ManagedIdentity,
   DBforPostgreSQL, DocumentDB and ServiceBus. Terraform auto-registration is off
   so a read-only plan never registers providers.
3. Run `make infra-fmt` and `make infra-validate ENV=test`.
4. Run `make infra-init ENV=test ROOT=bootstrap`, then `make infra-plan ENV=test
   ROOT=bootstrap`. Inspect the saved plan with `terraform show
   infra/bootstrap/test.tfplan`. After approval, run `make infra-apply ENV=test
   ROOT=bootstrap`. First bootstrap uses a local backend with `test.tfstate`;
   provider creates containers over ARM and grants operator container-scoped blob
   permissions. Allow RBAC propagation before migration.

## Migrate bootstrap state

After bootstrap apply, copy the platform backend example to
`infra/environments/test/bootstrap.backend.hcl`; change container to `bootstrap`
and key to `test/bootstrap.tfstate`. Replace ignored `infra/bootstrap/backend.tf`
with:

```hcl
terraform {
  backend "azurerm" {}
}
```

Run from repository root (the same TF_DATA_DIR is essential):

```sh
TF_DATA_DIR="$PWD/infra/bootstrap/.terraform/test" terraform -chdir=infra/bootstrap init -migrate-state -backend-config=../environments/test/bootstrap.backend.hcl
```

Review and answer Terraform's migration prompt. Verify remote state and run a
no-change bootstrap plan before retiring local copies according to retention
policy. Never copy an existing environment's state into another environment.
The Azure backend uses blob leases for locking. Exercise two competing plans
against a deliberately held lease in Test; never force-unlock an active apply.
Versioning and 30-day blob/container recovery are configured; test restoration
before treating recovery as verified. `prevent_destroy` requires a deliberate
code change to remove data services; it is not an Azure deletion lock.

## Platform

Run `make infra-init ENV=test ROOT=platform`, `make infra-plan ENV=test
ROOT=platform`, inspect the saved plan, then `make infra-apply ENV=test
ROOT=platform`. Apply only a freshly reviewed plan for the intended environment.
Run another plan after apply; investigate all unexpected drift. Export
`terraform -chdir=infra/platform output -json handoff` using the environment's
TF_DATA_DIR into ignored `infra/outputs/` for the next phase.

Workloads never get platform-state access. The currently prepared GitHub
federation uses `repo:TeesideCoderProMax/resource.center:environment:test`.
Repository access and environment protections could not be verified. Before
running a workflow, restrict permitted branches and configure gated staging /
production environments. Phase 3 creates delivery workflows, not this phase.

## PostgreSQL privileges and migration path

The configured human Entra administrator must run
`scripts/initialize-loan-roles.sql` from a runner that resolves the private DNS
zone and reaches TCP 5432. Obtain an Entra token for
`https://ossrdbms-aad.database.windows.net`; use TLS `verify-full` with trusted
roots. Do not print the token. The SQL requires psql variables named in its header
and separates migration schema ownership from Loan DML; publisher gets no table
rights until the outbox migration grants SELECT/UPDATE on specific tables.
No Notifications principal is created.

For CI, phase 2 builds a short-lived **manual Container Apps Job** inside this
same environment, with only the migrations identity attached, to run versioned
migrations. An external GitHub runner cannot connect directly to private PG.
Initial role setup still requires administrator access: use an operator private
runner (VPN/peering), or deliberately configure a temporary bootstrap job
identity as PostgreSQL Entra administrator, execute setup, restore the configured
human administrator and remove that job/identity. Never make the everyday
migration or runtime identity permanent server administrator. The temporary
bootstrap path was executed for Test using a dedicated disposable identity;
`infra/diagnostics` contains the operator-only scripts. The regular migrations
identity was not made a server administrator.

SDK clients in phase 2 must select their own user-assigned identity client ID,
refresh database access tokens, and close/renew connections appropriately.
Required notification secret name: `resend-api-key` (populate out of band into
the notification-only vault). Auth0 public issuer/audience are configuration;
any future confidential client secret needs a separate scope. No secrets exist
yet and Terraform does not create secret values.

## Runtime acceptance after apply

Use disposable, no-email diagnostic jobs with each identity attached separately.
Record timestamp, identity, action and sanitized outcome. Test PostgreSQL DNS/TLS,
Loan DML without DDL, migration DDL, publisher outbox-only access; Notifications
must fail PostgreSQL authentication. In Cosmos, Notifications must create/read,
ETag-update and detect duplicate test jobs using `(eventId, id)`; Loan and publisher
must fail. Publisher must send a synthetic queue request and fail receive;
Notifications must receive/settle it and fail send. The real email worker must
not be running for these synthetic requests. Verify registry pulls, notification
secret reads, and denied Loan vault access. Delete diagnostic data/jobs afterward.
Exercise lease locking, state-container isolation and restoration. None of these
checks is replaced by Terraform validation or a speculative plan.

## Student/free-tier-first defaults (2026-10-29)

Test selects `cosmos_capacity_mode="free_tier"` (400 RU/s shared database,
1,000 RU/s account ceiling) and `registry_sku="Standard"`. Cosmos's lifetime
free allowance covers 1,000 RU/s and 25 GB; storage overage remains billable.
Recheck the subscription's single free-tier slot and first-year PostgreSQL/ACR
eligibility before apply. No paid fallback is automatic. Explicit serverless mode
has no free-tier discount; review any post-deployment mode change as a migration.
See `docs/cloud/test-costs.md`. Additional environments cannot each claim the same
subscription's Cosmos free slot; choose their capacity deliberately before apply.
