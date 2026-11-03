"""Bounded public contract smoke. Live login/email proof is a separate check."""
import json
import os
import time
import urllib.error
import urllib.request
import uuid


def request(url, body=None, correlation=None):
    headers = {"Content-Type": "application/json"}
    if correlation:
        headers["x-correlation-id"] = correlation
    req = urllib.request.Request(url, data=body, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=15) as response:
            return response.status, json.load(response), response.headers
    except urllib.error.HTTPError as error:
        result = error.code, {}, error.headers
        error.close()
        return result


def check(api, web, version, allow_unconfigured=False):
    correlation = str(uuid.uuid4())
    code, live, headers = request(api + "/health/live", correlation=correlation)
    assert code == 200 and live["status"] == "ok", "Loan liveness failed"
    assert headers.get("x-correlation-id") == correlation, "Correlation header lost"
    code, ready, _ = request(api + "/health/ready")
    assert code == 200 and ready["status"] == "ready" and ready["database"] == "reachable", "Database not ready"
    assert ready["version"] == version, "Wrong backend version"
    code, catalogue, _ = request(api + "/api/v1/catalogue")
    assert code == 200, "Catalogue failed"
    # Catalogue wraps model rows in the items array.
    models = catalogue["items"]
    assert models and all(m["loanDurationDays"] == 2 for m in models), "Invalid or empty catalogue"
    code, _, _ = request(api + "/api/v1/reservations", b'{"modelId":"dell-latitude-7440"}')
    assert code == 401 or (allow_unconfigured and code == 503), "Unauthenticated write did not fail closed"
    code, health, _ = request(web + "/api/health")
    assert code == 200 and health["status"] == "ok" and health["version"] == version, "Web health/version mismatch"
    if not allow_unconfigured:
        assert ready["auth0Configured"] and health["auth0Configured"], "Auth0 configuration is missing"
        code, web_ready, _ = request(web + "/api/ready")
        assert code == 200 and web_ready["loanApi"] == "reachable", "Web cannot reach the Loan API"


def main():
    for attempt in range(12):
        try:
            check(os.environ["LOAN_API_URL"].rstrip("/"), os.environ["WEB_URL"].rstrip("/"),
                  os.environ["EXPECTED_VERSION"], os.getenv("ALLOW_UNCONFIGURED_AUTH") == "true")
            print("Public smoke passed; live Auth0 and email remain separate checks")
            return
        except (AssertionError, KeyError, ValueError, OSError) as error:
            if attempt == 11:
                raise
            print(f"Smoke retry {attempt + 1}: {error}")
            time.sleep(5)


if __name__ == "__main__":
    main()
