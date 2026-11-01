# Commands
Source of truth: root Makefile and infra/scripts/terraform.sh.
- Full local checks: make check (Rust fmt/clippy/tests, Next.js typecheck/build/audit, Terraform fmt/validation for all environments).
- Focused: make backend-fmt, backend-lint, backend-test; make backend-test-postgres runs isolated local PostgreSQL migration/transaction tests (Docker + OpenSSL); make frontend-typecheck, frontend-build, frontend-audit; make docker-build.
- Deployed smoke: EXPECTED_VERSION=<intended-release> make smoke-test. Get the version from the intended release, not an old memory.
- Terraform: make infra-fmt; make infra-validate ENV=test; make infra-init / infra-plan / infra-apply ENV=<environment> ROOT=<bootstrap|platform|workloads>. Apply only within authorized scope.
- Memory references: serena memories check.
