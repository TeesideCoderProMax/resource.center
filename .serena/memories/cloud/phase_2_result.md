# Workload evidence and limits
Stored evidence date 2026-10-31; current source reviewed 2026-11-01.
- Sources: docs/cloud/phase-2-handoff.md, test-workloads-outputs.json, test-workloads-checks.json, ADR 0003; backend/, frontend/, infra/workloads/.
- Test has web and Loan API Container Apps with managed HTTPS; no-ingress Notifications with KEDA; scheduled publisher and private migration jobs. Web hosting uses Container Apps because subscription region policy blocked Static Web Apps.
- Versioned migration creates Loan schema/outbox and publisher grants; deterministic demo seeding is separate. Worker writes Cosmos jobs with ETags; queue scale-from-zero and identity isolation checks are recorded.
- Loan routes implement catalogue/reservation/subscription/collection/return. Browser currently supplies catalogue only; Auth0 configuration flags do not implement login. Missing API auth configuration returns 503 AuthNotConfigured.
- Recorded notification results use fake delivery. Resend mode exists in ConfigurableEmailSender but live receipt is not verified.
- Current image digests and endpoints belong in outputs/handoff, not memory. Further results: `mem:cloud/phase_3_result`; gaps: `mem:assessment/requirements`.
