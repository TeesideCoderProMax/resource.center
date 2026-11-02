#!/usr/bin/env bash
# Copy AUTH0_CLIENT_SECRET and AUTH0_SECRET from an env file into the web Key Vault
# without printing them or passing them on the command line.
# Usage: infra/scripts/set-web-secrets.sh <vault-name> [env-file]
set -euo pipefail
vault=${1:?vault name required, e.g. rctest0330980fwkv}
env_file=${2:-frontend/.env.local}
tmp=$(mktemp)
chmod 600 "$tmp"
trap 'rm -f "$tmp"' EXIT
for pair in AUTH0_CLIENT_SECRET:auth0-client-secret AUTH0_SECRET:auth0-secret; do
  key=${pair%%:*} name=${pair#*:}
  # Value after the first '=', minus any trailing inline comment and whitespace.
  sed -n "s/^${key}=//p" "$env_file" | head -n1 | sed 's/[[:space:]]*#.*$//; s/[[:space:]]*$//' | tr -d '\n' > "$tmp"
  if [[ ! -s "$tmp" ]]; then echo "$key is empty in $env_file" >&2; exit 1; fi
  az keyvault secret set --vault-name "$vault" --name "$name" --file "$tmp" --encoding utf-8 --query name -o tsv
done
