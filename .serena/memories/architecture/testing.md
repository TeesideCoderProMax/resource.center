# Tests and evidence
Reviewed 2026-11-01; source files and stored results, not a fresh test run.
- make backend-test runs Cargo tests; HTTP tests use fake repositories/token validation, worker tests use fake Cosmos/email ports. CI YAML invokes tests, but successful GitHub runs need separate evidence.
- docs/cloud/test-phase3-checks.json records 10 passing Rust tests, real PostgreSQL 5-way last-unit contention, publisher lease recovery, three event types through Service Bus/Cosmos with fake email, rollback and 40/40 catalogue HTTP 200 responses. docs/cloud/test-workloads-checks.json records worker scale-from-zero.
- Do not call fake-auth rejection a live Auth0 HTTP 403, fake delivery a received email, or a burst of HTTP 200s a full scale-out/performance study.
- Required next evidence: reservation-time due date; full JWT/role failures and valid flows; multi-subscriber return; rollback after partial DB work; concurrent worker duplicates; email-send/crash and queue lock expiry; broker/email outage plus backlog recovery; malformed/unsupported messages; explicit load targets/results.
- Test business rules through ports; use real PostgreSQL for contention/transactions and real Cosmos for ETag/partition behaviour. Fake ports do not prove vendor integration.
- Assessment targets: `mem:assessment/cloud_module_criteria`. Deployment checks and limitations: `mem:cloud/phase_3_result`.
