# Layers
- Source: `docs/architecture/2026-10-25-resource-center-handoff.md`, user-approved design dated 2026-10-25; implementation unverified.
- Both services: presentation → application → domain; infrastructure implements ports; outer composition roots wire adapters/configuration.
- Domain entities/value objects/invariants independent of FastAPI/PostgreSQL/Cosmos/Service Bus/Resend SDKs.
- Application use cases inject repositories, transaction/UoW boundaries, publishing/delivery ports, clocks/IDs only as needed; vendor models remain outside core.
- HTTP routes and queue consumers validate/map input/results; business rules stay in domain/use cases.
- Loan return use case selects recipients and saves transition plus outbox requests in a local transaction. Notification use case handles job state and delivery via its own ports.
