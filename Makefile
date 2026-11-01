.PHONY: check backend-fmt backend-lint backend-test frontend-typecheck frontend-build frontend-audit docker-build smoke-test infra-fmt infra-validate infra-init infra-plan infra-apply

check: backend-fmt backend-lint backend-test frontend-typecheck frontend-build frontend-audit infra-fmt
	$(MAKE) infra-validate ENV=test
	$(MAKE) infra-validate ENV=staging
	$(MAKE) infra-validate ENV=production

backend-fmt:
	cargo fmt --all --manifest-path backend/Cargo.toml -- --check

backend-lint:
	cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings

backend-test:
	cargo test --manifest-path backend/Cargo.toml

frontend-typecheck:
	npm --prefix frontend run typecheck

frontend-build:
	npm --prefix frontend run build

frontend-audit:
	npm --prefix frontend audit --omit=dev

docker-build:
	docker build -t rc-backend:ci backend
	docker build -t rc-web:ci frontend

smoke-test:
	./infra/scripts/smoke-test.sh

infra-fmt:
	terraform fmt -check -recursive infra

infra-validate:
	@test -n "$(ENV)" || (echo 'Set ENV=test|staging|production'; exit 2)
	./infra/scripts/terraform.sh validate bootstrap "$(ENV)"
	./infra/scripts/terraform.sh validate platform "$(ENV)"
	./infra/scripts/terraform.sh validate workloads "$(ENV)"

infra-init infra-plan infra-apply:
	@test -n "$(ENV)" -a -n "$(ROOT)" || (echo 'Set ENV and ROOT=bootstrap|platform|workloads'; exit 2)
	./infra/scripts/terraform.sh $(patsubst infra-%,%,$@) "$(ROOT)" "$(ENV)"

