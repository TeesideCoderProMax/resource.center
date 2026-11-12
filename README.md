# Resource Center / Campus Device Loan

Students browse a campus catalogue, add devices to a loan bag and reserve them for
1–14 days. Staff hand devices over, record returns and see overdue loans and stock.
Service ownership and selected design: [AGENTS.md](AGENTS.md).

## Architecture

Next.js web app (with a same-origin proxy to the API) · Auth0 identity · Rust Loan
API with PostgreSQL and a transactional outbox/publisher · Azure Service Bus ·
independent Rust Notifications worker with Cosmos DB and optional Resend email.
Delivery is at-least-once; there is no exactly-once email claim.

## Current state (2026-11-12)

- **App:** 36-model catalogue with images, specs and credits (`/image-credits`),
  device pages, loan bag with multi-item reservation, My Loans, and a staff desk
  (collection queue, returns, overdue highlighting, inventory, CSV export).
  Role checks apply on both the web proxy and the API.
- **Loans:** borrowers choose 1–14 whole days; the API validates the period and saves
  the due date. See [catalogue and loan periods](docs/backend/catalogue-and-loan-periods.md).
- **Local checks:** Rust workspace and PostgreSQL tests, 40 frontend unit tests,
  production build and 15 Playwright e2e tests (including phone width) pass.
- **Azure Test:** deployed on Container Apps in France Central; the release smoke
  checks and catalogue checks passed (evidence in [docs/cloud](docs/cloud/)).
  Staging and Production are templates only, not deployed.
- **Open:** live Auth0 tenant configuration and live email, real Cosmos and deployed
  recovery checks, promotion gates, and final report and media.
  See the [phase 4 handoff](docs/backend/phase-4-handoff.md).

## Run and check

Install the pinned Rust toolchain, Node 22, Docker and Terraform.

```sh
npm --prefix frontend ci
make dev                          # local PostgreSQL, Loan API and web (uses frontend/.env.local)
make check backend-test-postgres  # full local checks; PostgreSQL runs in a throwaway container
make docker-build
```

[Release runbook](docs/backend/release-runbook.md) ·
[Submission outline and checklist](docs/assessment/submission-checklist.md).
The final ZIP readme requires the student's supplied name and user ID; the packager
adds them. This repository readme does not assert a verified submission identity.
