# Phase 2 ownership

This directory is reserved for phase 2; there are no deployable workloads yet.
Use the same pinned provider and a separate Azure backend container `workloads`,
key `<environment>/workloads.tfstate`. Consume the exported `handoff` JSON as an
explicit input, never `terraform_remote_state` for bootstrap/platform.

Deploy only in the workload resource group. Platform owns the environment,
registry, networks, data services, managed identities and their role assignments.
The GitHub workload identity can attach these identities and consequently execute
with their data access: it is a trusted deployer, not an untrusted CI identity.
Do not grant its OIDC environment to pull requests or untrusted branches.

## Backend stack revision — 2026-10-30

The accepted backend is now Rust + Tokio + Axum: Axum/Tokio Loan API,
Rust/Tokio publisher and independent Notifications worker. See
`docs/architecture/adr/0002-rust-backend.md`. The deployed phase 1 resources
and exported interface require no change; phase 2 owns Rust images and adapter
integration tests. Existing Python diagnostics remain operator tooling.
