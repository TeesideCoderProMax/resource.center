# Foundation evidence and constraints
Stored evidence date 2026-10-30; current source reviewed 2026-11-01.
- Sources: docs/cloud/phase-1-handoff.md, test-foundation-outputs.json, test-foundation-checks.json; infra/bootstrap and infra/platform. Runtime checks are historical evidence, not current live state.
- Azure Test: France Central; private PostgreSQL 16 with Entra auth; Cosmos free-tier shared 400 RU/s, /eventId partition; Service Bus Basic; Standard ACR; Key Vault; Container Apps environment. Resolve exact names/IDs from outputs rather than duplicate them here.
- Terraform remote state separates bootstrap/platform/workloads and environments; narrow data access and recovery checks recorded. Application/migration/publisher identities have distinct PostgreSQL roles; Notifications denied PostgreSQL; Loan denied Cosmos.
- Cosmos free-tier slot is subscription-limited; do not assume other environments share eligibility. PostgreSQL and provisioned Cosmos have fixed capacity; budget alerts are not spending caps.
- Temporary diagnostics cleanup is recorded in handoff. Database restore drills, retention choices and current cost/credit checks are not proved by old diagnostics.
- Current workloads and grants: `mem:cloud/phase_2_result`.
