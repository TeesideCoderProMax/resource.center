# Layer guidance
- User-required guidance (2026-10-25), not implemented architecture.
- Proposed per-capability layers: presentation → application → domain; infrastructure implements application ports; outer composition root wires concrete adapters.
- Domain independent of HTTP, database SDKs, cloud runtimes. Inject repositories, clocks, IDs, external gateways as needed; keep vendor models/configuration outside core.
- Concrete repository examples await application sources.
