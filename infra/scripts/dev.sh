#!/usr/bin/env bash
# `make dev`: PostgreSQL 17 (TLS) + Loan API + web from the local images, kept running
# until Ctrl-C. Publisher and Notifications need Azure Service Bus/Cosmos and are not
# started. Without Auth0 env vars the stack is signed-out only (writes fail closed).
set -euo pipefail
cd "$(dirname "$0")/../.."
# Reuse the web's local settings (Auth0 etc.) so `make dev` signs in like `npm run dev`.
# The containers still get their own LOAN_API_BASE_URL and APP_BASE_URL below.
if [[ -f frontend/.env.local ]]; then
  set -a
  # shellcheck source=/dev/null
  source frontend/.env.local
  set +a
fi
prefix="rc-dev"
api_port="${DEV_API_PORT:-8080}"
web_port="${DEV_WEB_PORT:-3000}"
cleanup() {
  echo "Stopping $prefix stack"
  docker rm -f "$prefix-web" "$prefix-api" "$prefix-pg" >/dev/null 2>&1 || true
  docker network rm "$prefix" >/dev/null 2>&1 || true
  rm -rf "${cert_dir:-}"
}
# Remove leftovers from an interrupted run before starting.
cleanup >/dev/null
cert_dir=$(mktemp -d)
trap cleanup EXIT INT TERM
chmod 755 "$cert_dir"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -keyout "$cert_dir/server.key" -out "$cert_dir/server.crt" -subj '/CN=pg' -addext 'subjectAltName=DNS:pg' >/dev/null 2>&1
docker network create "$prefix" >/dev/null
docker run -d --name "$prefix-pg" --network "$prefix" --network-alias pg -e POSTGRES_PASSWORD=local-dev -e POSTGRES_DB=loan -v "$cert_dir:/certs:ro" postgres:17 \
  sh -c 'cp /certs/server.* /tmp/; chown postgres:postgres /tmp/server.*; chmod 600 /tmp/server.key; exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key' >/dev/null
for _ in {1..40}; do
  docker exec "$prefix-pg" pg_isready -h 127.0.0.1 -U postgres >/dev/null 2>&1 && break
  sleep 0.5
done
pg_args=(--network "$prefix" -e PGHOST=pg -e PGUSER=postgres -e PGDATABASE=loan
  -e PGPASSWORD=local-dev -e SSL_CERT_FILE=/certs/server.crt -v "$cert_dir:/certs:ro")
docker run --rm "${pg_args[@]}" -e SEED_DEMO_DATA=true rc-backend:ci migrate

# Optional Auth0 settings are passed through from the caller's environment.
auth_args=()
for key in AUTH0_DOMAIN AUTH0_AUDIENCE AUTH0_ISSUER AUTH0_ROLES_CLAIM AUTH0_EMAIL_CLAIM; do
  [[ -n "${!key:-}" ]] && auth_args+=(-e "$key")
done
web_auth=()
for key in AUTH0_DOMAIN AUTH0_CLIENT_ID AUTH0_CLIENT_SECRET AUTH0_SECRET AUTH0_AUDIENCE AUTH0_ROLES_CLAIM; do
  [[ -n "${!key:-}" ]] && web_auth+=(-e "$key")
done
# Local signed-out stack only: lets /api/ready pass without an Auth0 tenant.
[[ ${#web_auth[@]} -eq 0 ]] && web_auth+=(-e ALLOW_UNCONFIGURED_AUTH=true)
docker run -d --name "$prefix-api" --network-alias api "${pg_args[@]}" "${auth_args[@]}" \
  -p "127.0.0.1:$api_port:8080" -e SERVICE_VERSION=dev -e CORS_ALLOWED_ORIGINS="http://localhost:$web_port" rc-backend:ci api >/dev/null
docker run -d --name "$prefix-web" --network "$prefix" "${web_auth[@]}" -p "127.0.0.1:$web_port:3000" \
  -e RC_VERSION=dev -e LOAN_API_BASE_URL=http://api:8080 -e APP_BASE_URL="http://localhost:$web_port" rc-web:ci >/dev/null

cat <<EOF

Web:      http://localhost:$web_port   (ready: /api/ready)
Loan API: http://localhost:$api_port/api/v1/catalogue
Not started: publisher and Notifications (need Azure Service Bus/Cosmos).
Ctrl-C stops and removes everything.

EOF
docker logs -f "$prefix-api" 2>&1 | sed 's/^/[api] /' &
docker logs -f "$prefix-web" 2>&1 | sed 's/^/[web] /'
