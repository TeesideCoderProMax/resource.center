# Boundaries
- User-required guidance (2026-10-25): business capabilities, explicit ownership, loose coupling; avoid entity-only services, frontend business orchestration, shared private schemas, unnecessary synchronous chains.
- Consider events/local projections with explicit consistency and operational costs.
- No services or authoritative data owners established in this empty repository.
- If Campus Device Loan identity is confirmed, inspect notification access to loans/outbox/waitlists; propose explicitly shared bounded context versus independent notification context with owned storage/event-fed projections. User decision required; neither accepted.
- Distinguish C4 containers, deployment containers, Cosmos containers.
