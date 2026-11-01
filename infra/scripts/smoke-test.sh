#!/usr/bin/env bash
set -euo pipefail

LOAN_API_URL="${LOAN_API_URL:-https://rc-test-0330980f-loan-api.livelywave-2cc0564d.francecentral.azurecontainerapps.io}"
WEB_URL="${WEB_URL:-https://rc-test-0330980f-web.livelywave-2cc0564d.francecentral.azurecontainerapps.io}"
EXPECTED_VERSION="${EXPECTED_VERSION:-}"

echo "==> Checking Loan API liveness (${LOAN_API_URL}/health/live)"
live_json="$(curl -fsS --retry 3 --retry-delay 2 --max-time 45 "${LOAN_API_URL}/health/live")"
echo "    ${live_json}"

echo "==> Checking Loan API readiness (${LOAN_API_URL}/health/ready)"
ready_json="$(curl -fsS --retry 3 --retry-delay 2 --max-time 45 "${LOAN_API_URL}/health/ready")"
echo "    ${ready_json}"
echo "${ready_json}" | grep -q '"status":"ready"'

if [[ -n "${EXPECTED_VERSION}" ]]; then
  echo "==> Verifying Loan API service version matches ${EXPECTED_VERSION}"
  matched=false
  for _ in $(seq 1 12); do
    if echo "${ready_json}" | grep -q "\"version\":\"${EXPECTED_VERSION}\""; then
      matched=true
      break
    fi
    sleep 2
    ready_json="$(curl -fsS --retry 3 --retry-delay 2 --max-time 45 "${LOAN_API_URL}/health/ready")"
  done
  echo "    ${ready_json}"
  if [[ "${matched}" != "true" ]]; then
    echo "Expected version ${EXPECTED_VERSION}, got ${ready_json}" >&2
    exit 1
  fi
fi

echo "==> Checking public catalogue (${LOAN_API_URL}/api/v1/catalogue)"
catalogue_json="$(curl -fsS --retry 3 --retry-delay 2 --max-time 45 "${LOAN_API_URL}/api/v1/catalogue")"
echo "${catalogue_json}" | grep -q '"loanDurationDays":2'

echo "==> Checking unauthenticated write rejection (HTTP 401 or 503 when Auth0 unconfigured)"
unauth_code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 30 -X POST "${LOAN_API_URL}/api/v1/reservations" -H 'content-type: application/json' -d '{"modelId":"dell-latitude-7440"}')"
if [[ "${unauth_code}" != "401" && "${unauth_code}" != "503" ]]; then
  echo "Expected HTTP 401 or 503 on unauthenticated POST /api/v1/reservations, got ${unauth_code}" >&2
  exit 1
fi

echo "==> Checking Web health (${WEB_URL}/api/health)"
web_health="$(curl -fsS --retry 3 --retry-delay 2 --max-time 45 "${WEB_URL}/api/health")"
echo "    ${web_health}"
echo "${web_health}" | grep -q '"status":"ok"'

echo "==> All smoke checks passed."
