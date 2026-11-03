import unittest
from unittest.mock import patch
from test_release import load
smoke = load("smoke")

class SmokeTests(unittest.TestCase):
    def response(self, url, body=None, correlation=None):
        if url.endswith("/health/live"):
            return 200, {"status": "ok"}, {"x-correlation-id": correlation}
        if url.endswith("/health/ready"):
            return 200, {"status": "ready", "database": "reachable", "version": "sha", "auth0Configured": True}, {}
        if url.endswith("/api/ready"):
            return 200, {"status": "ready", "loanApi": "reachable", "auth0Configured": True}, {}
        if url.endswith("/catalogue"):
            return 200, {"items": [{"loanDurationDays": 2}]}, {}
        if body:
            return 401, {}, {}
        return 200, {"status": "ok", "version": "sha", "auth0Configured": True}, {}

    def test_contract_and_release_identity(self):
        with patch.object(smoke, "request", side_effect=self.response):
            smoke.check("https://api", "https://web", "sha")
            with self.assertRaises(AssertionError):
                smoke.check("https://api", "https://web", "wrong")

    def test_unconfigured_auth_requires_explicit_fake_scope(self):
        def unconfigured(url, **kwargs):
            code, body, headers = self.response(url, **kwargs)
            if "auth0Configured" in body:
                body["auth0Configured"] = False
            return code, body, headers
        with patch.object(smoke, "request", side_effect=lambda url, body=None, correlation=None: unconfigured(url, body=body, correlation=correlation)):
            with self.assertRaises(AssertionError):
                smoke.check("https://api", "https://web", "sha")
            smoke.check("https://api", "https://web", "sha", True)
