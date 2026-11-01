# Layers
- Presentation → application → domain; infrastructure implements ports; composition roots inject adapters. Keep framework/vendor types outside domain/application APIs.
- Loan phase 1 now runs reserve/subscribe/collect/return use cases in application.rs over LoanStorePort and LoanTransactionPort. Domain owns availability and state transitions; PostgreSQL owns SQL, model-first locks and commit/rollback. The fake runs the same use cases with copy-on-success storage.
- Loan selects recipients and writes state/outbox atomically. Notifications uses job-store/email ports; owned worker leases remain phase 2 work.
- Local implementation/check evidence and limits: docs/backend/phase-1-handoff.md. Contracts and transaction ordering: docs/backend/contracts.md. No phase 1 deployment.
