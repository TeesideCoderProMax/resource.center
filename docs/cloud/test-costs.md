# Test costs: free-tier first

Updated 2026-10-29 following the user's instruction to prioritize free allowances
and their statement that the full USD 100 student credit remains. Credit balance
is user-reported, not verified through billing. The Test foundation is deployed
as of 2026-10-30; live Cosmos free-tier enablement was verified.
France Central remains the proposed region. All quoted retail rates are USD,
before tax/discounts, not a spending cap.

## Prepared selections

| Service | Prepared configuration | Free allowance / limitation |
|---|---|---|
| PostgreSQL | B1ms, 32 GiB storage, seven-day backup | Eligible first-12-month offer: 750 hours/month, 32 GB data and 32 GB backup. Verify benefit age/usage; excess backup can cost extra. |
| ACR | Standard | Eligible first-12-month offer: one Standard registry, 100 GB storage. Changed from Basic to match the offer; Standard costs more after eligibility ends. |
| Cosmos | Free-tier account; database shared manual 400 RU/s; one region | Lifetime allowance 1,000 RU/s and 25 GB per subscription; free tier explicitly enabled on creation. Account throughput limit set to 1,000 RU/s. Storage beyond 25 GB remains chargeable. |
| Container Apps | Consumption | Monthly subscription-wide allowance: 180,000 vCPU-seconds, 360,000 GiB-seconds and two million requests. No workloads deployed yet. |
| Other foundations | State storage, private DNS, vault, logs, Basic Service Bus | Do not assume zero cost. Monitor eligible allowances and actual usage; remaining usage consumes credit. |

Sources: [Azure for Students](https://azure.microsoft.com/en-us/free/students/),
[Cosmos free tier](https://learn.microsoft.com/en-us/azure/cosmos-db/free-tier).

Azure inventory on 2026-10-29 found only `lab4-devops-cosmos` in `lab4-devops`:
`enableFreeTier=false`, `EnableServerless`. The slot was therefore
available before deployment and is now used by Resource Center.
The lab account was not modified. Resource Center receives its own Notifications
account, preserving service ownership. No free-tier discount applies to serverless.

`cosmos_capacity_mode="free_tier"` is the default. If the slot is unavailable,
deployment should fail, not silently select paid throughput. `serverless` is an
explicit alternative. Changing capacity mode after deployment requires migration
review; no in-place conversion is promised. The throughput ceiling does not cap
storage, network or ancillary costs. Keep one shared-throughput database rather
than assigning fresh dedicated throughput to each container.

## Serverless per 1,000 operations

Azure's [Retail Prices API](https://prices.azure.com/api/retail/prices) returned
France Central `Azure Cosmos DB serverless`, SKU `RUs`, meter `1M RUs` at
**USD 0.297 per million RU** on 2026-10-29.

Cost = operations × average RU per operation × 0.297 / 1,000,000.

| 1,000 operations | Assumed RU each | Request charge |
|---|---:|---:|
| Point-read a roughly 1-KB item by ID + partition key, session consistency | 1 | $0.000297 |
| Writes averaging 5 RU (illustrative, not a guaranteed write price) | 5 | $0.001485 |
| Queries/updates averaging 10 RU (illustrative) | 10 | $0.002970 |

This is database-operation cost only. Larger documents, indexing, query shape,
consistency and retries change RU use; one notification requires several database
operations. Storage, backups where chargeable, networking and other services are
additional. Measure `x-ms-request-charge` in integration tests.
[Microsoft RU explanation](https://learn.microsoft.com/en-us/azure/cosmos-db/request-units).

## Retail fallback and deployment gates

The previous ~$23.13/month base was **B1ms + 32 GB PostgreSQL + Basic ACR before
benefits**, not an account-specific bill. It is no longer the prepared SKU mix.
Observed retail reference rates were $0.019/hour PostgreSQL compute,
$0.133/GB-month PostgreSQL storage and $0.1666/day Basic ACR. Reprice Standard ACR
and all services before deploying if the 12-month allowances have expired.
Annual student credit renewal does not establish renewal of first-year benefits.

Verify the free-service expiry/remaining allowance in the subscription and record
it before apply. Prefer the matching free SKU while eligible; after expiry review
Basic ACR and database costs. Do not silently upgrade to PAYG, consume credit for
provisioned Cosmos without its discount, or alter unrelated resources.

Set a budget alert amount and recipient before deployment; these remain missing.
Alerts notify, not enforce a custom cap. Student credit exhaustion normally
disables services unless the subscription is upgraded; see
[Microsoft's student-credit guidance](https://learn.microsoft.com/en-us/azure/cost-management-billing/manage/azurestudents-subscription-disabled).
