# Disposable foundation checks

These scripts verify infrastructure; they implement no business features and
never call Resend. Run only against an isolated Test foundation with no email
worker consuming the diagnostic queue. They use Azure managed identity tokens,
never passwords. Logs contain check names/results, not tokens.

`check.py` supports bootstrap, migrations, Loan, publisher, Notifications and
cleanup modes. `state_check.py` runs as the workload deployer and verifies own
state read/write/delete plus denied bootstrap/platform state. `run.py` submits
manual jobs through the documented Microsoft.App/jobs 2025-01-01 API because the
preview CLI creation path timed out. It uses the CLI for start/status/logs/delete.
No automatic retry or recurring schedule exists; timeout is 180 seconds.

Build `Dockerfile`, push to the project registry, and record the image digest in
ignored `infra/outputs/test-diagnostic-image.txt`. The 2026-10-30 run used a local
base image already built from this Dockerfile, then copied the updated check.py
after Docker Hub temporarily timed out. The final immutable image digest is in
the phase 1 handoff. Dependencies are pinned in requirements.txt; production
application images are separate phase 2 artifacts.

Required ignored inputs under `infra/outputs`:

- `test-handoff.json`: `terraform output -json handoff` from the Test platform.
- `test-diagnostic-image.txt`: full ACR image reference including digest.
- `test-diagnostic-run-id.txt`: unique synthetic queue message ID for this run.
- `test-diagnostic-admin.json`: temporary bootstrap identity's id, client_id,
  principal_id and name (no credentials).
- `test-deployer.json`: existing workload deployer's same identity fields.

Initial role setup requires a dedicated temporary identity with ACR pull and a
**temporary** PostgreSQL Entra administrator entry. Do not promote a runtime or
routine migrations identity to permanent administrator. Run bootstrap first,
then delete that administrator entry. The configured human administrator stays.
Use `python infra/diagnostics/run.py run MODE`, followed by `status MODE` and
`logs MODE`. Keep bootstrap logs before deleting its job. Run migrations before
Loan (it creates the disposable `loan.foundation_diagnostic` table), publisher
before Notifications (it sends the synthetic request), and cleanup after Loan.
Run deploy to verify state permissions. Check terminal execution status as well
as PASS logs; a start acknowledgement alone is not success.

Delete **every** diagnostic job with `delete MODE` afterward; remove the temporary
identity's ACR role and the temporary identity. Cleanup drops only the diagnostic
table. Cosmos test items and the diagnostic queue message are deleted/settled by
the tests. Soft-deleted storage sentinels follow backend retention. Retain the
small ACR image for reproducibility; no running compute is retained by that image.
These scripts require operator privileges and are not ordinary release workflows.
