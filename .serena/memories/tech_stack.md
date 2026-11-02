# Stack and tool sources
Reviewed 2026-11-01.
- Backend: Rust + Tokio + Axum; workspace backend/ with rc-shared, loan-service and notifications-worker. Publisher is a Loan command, not a separate business boundary. Versions/toolchain: Cargo.toml, Cargo.lock and rust-toolchain.toml.
- Frontend: Next.js + TypeScript, standalone container; package.json/package-lock.json own versions. Auth0 login via @auth0/nextjs-auth0 (server session, /api/loan proxy); no live tenant configured yet.
- Data/integration: PostgreSQL via tokio-postgres; Cosmos, Service Bus and Resend adapters; Auth0 JWT validation. Domain/application code should not depend on vendor SDKs.
- Azure Container Apps managed HTTPS; Terraform roots bootstrap/platform/workloads; GitHub Actions and Makefile. Persistent PostgreSQL and Test Cosmos are not scale-to-zero.
- Python scripts are operator diagnostics, not a second backend stack. Exact versions/audit results belong in manifests and dated evidence, not duplicated memory.
- Accepted topology: `mem:architecture/selected_baseline`; commands: `mem:suggested_commands`.
