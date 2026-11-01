#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
action=${1:?action required}
root=${2:?root required}
env=${3:?environment required}
case "$root" in bootstrap|platform|workloads) ;; *) echo 'Root must be bootstrap, platform, or workloads' >&2; exit 2;; esac
case "$env" in test|staging|production) ;; *) echo 'Explicit environment required: test, staging, production' >&2; exit 2;; esac
# Keep backend metadata isolated when changing environment; never use workspaces.
export TF_DATA_DIR="$PWD/infra/$root/.terraform/$env"
vars="$PWD/infra/environments/$env/$root.tfvars"
backend="$PWD/infra/environments/$env/$root.backend.hcl"
plan="$PWD/infra/$root/$env.tfplan"
case "$action" in
  validate)
    terraform -chdir="infra/$root" init -backend=false -input=false -lockfile=readonly
    terraform -chdir="infra/$root" validate ;;
  init)
    if [[ "$root" == bootstrap && ! -f infra/bootstrap/backend.tf ]]; then
      printf 'terraform {\n  backend "local" {}\n}\n' > infra/bootstrap/backend.tf
    fi
    if [[ "$root" == bootstrap ]] && grep -q 'backend "local"' infra/bootstrap/backend.tf; then
      terraform -chdir="infra/$root" init -input=false -lockfile=readonly -backend-config="path=$env.tfstate"
    else
      terraform -chdir="infra/$root" init -input=false -lockfile=readonly -backend-config="$backend"
    fi ;;
  plan)
    test -f "$vars"
    rm -f "$plan"
    terraform -chdir="infra/$root" plan -input=false -lock-timeout=60s -var-file="$vars" -var="environment=$env" -out="$plan" ;;
  apply)
    test -f "$plan"
    terraform -chdir="infra/$root" show -json "$plan" | python3 -c 'import json,sys; p=json.load(sys.stdin); sys.exit(0 if p["variables"]["environment"]["value"] == sys.argv[1] else "Plan environment mismatch")' "$env"
    terraform -chdir="infra/$root" apply -input=false -lock-timeout=60s "$plan" ;;
  *) echo 'Action must be init, validate, plan or apply' >&2; exit 2 ;;
esac
