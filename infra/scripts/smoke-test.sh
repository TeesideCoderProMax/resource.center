#!/usr/bin/env bash
set -euo pipefail
# Explicit endpoints prevent promotion from accidentally checking Test.
: "${LOAN_API_URL:?Set the target Loan URL}"
: "${WEB_URL:?Set the target web URL}"
: "${EXPECTED_VERSION:?Set the exact release version}"
python3 "$(dirname "$0")/smoke.py"
