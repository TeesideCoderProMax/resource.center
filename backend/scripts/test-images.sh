#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
prefix="rc-phase4-$$"
cert_dir=$(mktemp -d)
cleanup() {
  docker rm -f "$prefix-web" "$prefix-api" "$prefix-pg" >/dev/null 2>&1 || true
  docker network rm "$prefix" >/dev/null 2>&1 || true
  rm -rf "$cert_dir"
}
trap cleanup EXIT
chmod 755 "$cert_dir"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -keyout "$cert_dir/server.key" -out "$cert_dir/server.crt" -subj '/CN=pg' -addext 'subjectAltName=DNS:pg' >/dev/null 2>&1
docker network create "$prefix" >/dev/null
docker run -d --name "$prefix-pg" --network "$prefix" --network-alias pg   -e POSTGRES_PASSWORD=local-image-check -e POSTGRES_DB=loan -v "$cert_dir:/certs:ro" postgres:17   sh -c 'cp /certs/server.* /tmp/; chown postgres:postgres /tmp/server.*; chmod 600 /tmp/server.key; exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key' >/dev/null
# Probe TCP: the entrypoint's temporary init server answers on the Unix socket only.
ready=false
for _ in {1..40}; do
  if docker exec "$prefix-pg" pg_isready -h 127.0.0.1 -U postgres >/dev/null 2>&1; then ready=true; break; fi
  sleep 0.5
done
if [[ "$ready" != true ]]; then docker logs "$prefix-pg"; exit 1; fi
pg_args=(--network "$prefix" -e PGHOST=pg -e PGUSER=postgres -e PGDATABASE=loan
  -e PGPASSWORD=local-image-check -e SSL_CERT_FILE=/certs/server.crt -v "$cert_dir:/certs:ro")
# Exercise the actual runtime image and its embedded migrations/seed twice.
docker run --rm "${pg_args[@]}" -e SEED_DEMO_DATA=true rc-backend:ci migrate
docker run --rm "${pg_args[@]}" -e SEED_DEMO_DATA=true rc-backend:ci migrate
docker run -d --name "$prefix-api" --network-alias api "${pg_args[@]}"   -p 127.0.0.1::8080 -e SERVICE_VERSION=phase4-local rc-backend:ci api >/dev/null
docker run -d --name "$prefix-web" --network "$prefix" -p 127.0.0.1::3000   -e RC_VERSION=phase4-local -e LOAN_API_BASE_URL=http://api:8080 rc-web:ci >/dev/null
api_port=$(docker port "$prefix-api" 8080/tcp | awk -F: '{print $NF}')
web_port=$(docker port "$prefix-web" 3000/tcp | awk -F: '{print $NF}')
export LOAN_API_URL="http://127.0.0.1:$api_port" WEB_URL="http://127.0.0.1:$web_port"
# This isolated local check has no Auth0 tenant and sends no email.
EXPECTED_VERSION=phase4-local ALLOW_UNCONFIGURED_AUTH=true ./infra/scripts/smoke-test.sh
python3 infra/scripts/load-check.py "$LOAN_API_URL/api/v1/catalogue" --requests 40 --concurrency 5

# Verify all 36 catalogue assets in the actual frontend runtime container.
python3 - <<'CHECK_CATALOGUE'
import json, os, urllib.request
items = json.load(urllib.request.urlopen(os.environ['LOAN_API_URL'] + '/api/v1/catalogue'))['items']
assert len(items) == 36
for item in items:
    assert item['loanDurationDays'] == 14
    assert len(item['specifications']) >= 3
    with urllib.request.urlopen(os.environ['WEB_URL'] + item['imageUrl']) as response:
        assert response.status == 200
        assert response.read(4) == b'RIFF', item['id']
print('PASS: 36 runtime catalogue records and product images')
CHECK_CATALOGUE
