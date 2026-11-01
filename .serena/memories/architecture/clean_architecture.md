# Layers
Accepted design from handoff and ADR 0002; source reviewed 2026-11-01.
- Presentation → application → domain; infrastructure implements ports; composition roots inject adapters.
- Keep business decisions independent of Axum, runtime-specific Tokio mechanisms and vendor SDKs. HTTP/queue edges map inputs and outcomes; SQL adapters own locks and transaction mechanics.
- Loan selects recipients and saves state/outbox atomically. Notifications handles delivery state through job-store/email ports.
- Current source has domain rules and application ports, but Loan SQL adapter also decides availability and permitted return states. This is an implementation gap against the selected layering, not a named rubric requirement. Extract testable business decisions without weakening transaction guarantees.
