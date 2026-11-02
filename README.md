# Resource Center / Campus Device Loan

Next.js web app, Rust Loan API and outbox publisher, independent Rust Notifications,
PostgreSQL, Azure Service Bus and Cosmos DB. Auth0 supplies user identity; Resend
provides optional live email. Service ownership and selected design: [AGENTS.md](AGENTS.md).

Current status: backend phases 1–3 have local checks. Phase 4 adds a tested local
release path and evidence tools. Live Auth0/email, cloud rollout, independent gates
and final submission evidence remain open. See the [phase 4 handoff](docs/backend/phase-4-handoff.md).

Install the pinned Rust toolchain, Node 22, Docker and Terraform. Run `npm --prefix
frontend ci`, then `make check backend-test-postgres` and `make docker-build`.
The PostgreSQL check creates and removes a local test container; it does not use
cloud data. [Release runbook](docs/backend/release-runbook.md).

[Submission outline and checklist](docs/assessment/submission-checklist.md).
The final ZIP readme requires the student's supplied name and user ID; the packager
adds them. This repository readme does not assert a verified submission identity.
