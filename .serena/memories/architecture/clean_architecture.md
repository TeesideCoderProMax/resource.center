# Layers
- Presentation → application → domain; infrastructure implements ports; composition roots inject adapters. Keep framework/vendor types outside domain/application APIs.
- Loan phase 1 now runs reserve/subscribe/collect/return use cases in application.rs over LoanStorePort and LoanTransactionPort. Domain owns availability and state transitions; PostgreSQL owns SQL, model-first locks and commit/rollback. The fake runs the same use cases with copy-on-success storage.
- Loan selects recipients and writes state/outbox atomically. Presentation authenticates through AccessTokenVerifierPort (Auth0JwtVerifier adapter, FakeVerifier in tests). Notifications uses job-store/email/QueueConsumerPort ports; run_consumer_loop in application.rs owns the receive/settle loop, main.rs builds ServiceBusRestConsumer.
- Local implementation/check evidence and limits: docs/backend/phase-1-handoff.md. Contracts and transaction ordering: docs/backend/contracts.md. No phase 1 deployment.
