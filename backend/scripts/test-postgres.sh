#!/usr/bin/env bash
set -euo pipefail
# Disposable local database; never reads deployment credentials or touches cloud resources.
root=$(cd "$(dirname "$0")/../.." && pwd)
cert_dir=$(mktemp -d)
container="rc-phase1-test-$$"
cleanup() { docker rm -f "$container" >/dev/null 2>&1 || true; rm -rf "$cert_dir"; }
trap cleanup EXIT
chmod 755 "$cert_dir"
openssl req -x509 -newkey rsa:2048 -nodes -days 1 -keyout "$cert_dir/server.key" -out "$cert_dir/server.crt" -subj '/CN=localhost' -addext 'subjectAltName=DNS:localhost,IP:127.0.0.1' >/dev/null 2>&1
# The container copies the mounted key to a postgres-owned file with mode 600.
docker run -d --name "$container" -p 127.0.0.1::5432 \
    -e POSTGRES_PASSWORD=local-phase1-test -v "$cert_dir:/certs:ro" postgres:17 \
    sh -c 'cp /certs/server.* /tmp/; chown postgres:postgres /tmp/server.*; chmod 600 /tmp/server.key; exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/tmp/server.crt -c ssl_key_file=/tmp/server.key' >/dev/null
ready=false
for _ in {1..40}; do
    if docker exec "$container" pg_isready -U postgres >/dev/null 2>&1; then ready=true; break; fi
    sleep 0.5
done
if [[ "$ready" != true ]]; then docker logs "$container"; exit 1; fi
port=$(docker port "$container" 5432/tcp | awk -F: '{print $NF}')
SSL_CERT_FILE="$cert_dir/server.crt" RC_TEST_PG_PORT="$port" \
    cargo test --manifest-path "$root/backend/Cargo.toml" -p loan-service postgres_tests -- --ignored --nocapture
