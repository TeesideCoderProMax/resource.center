import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

release = load("release")
promotion = load("promotion")

class ReleaseTests(unittest.TestCase):
    def scenario(self, status="Succeeded", replicas=None, seed=True, state_environment=None, active=True):
        calls = []
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            config = root / "infra/environments/test"
            config.mkdir(parents=True)
            for name in ("workloads.tfvars", "workloads.backend.hcl"):
                (config / name).touch()
            def command(*args):
                calls.append(args)
                if "show" in args:
                    return json.dumps({"values": {"outputs": {"environment": {"value": state_environment}}}})
                if "output" in args:
                    return json.dumps({k: {"value": "https://example.test"} for k in ("loan_api_url", "web_app_url")})
                return ""
            def console(args, input, **kwargs):
                values = {"var.handoff.workload_resource_group": "rg", "var.name_prefix": "rc", "var.handoff.environment": "test", "var.seed_demo_data": seed}
                return json.dumps(values[input])
            def azure(*args):
                calls.append(args)
                if args[:2] == ("containerapp", "list"):
                    return [{"name": "rc-loan-api"}, {"name": "rc-notifications"}]
                if args[1:3] == ("revision", "list"):
                    return [{"name": "old", "properties": {"active": active}}]
                if args[1:3] == ("replica", "list"):
                    return replicas or []
                if args[1:3] == ("job", "start"):
                    return {"name": "migration-1"}
                if args[1:4] == ("job", "execution", "show"):
                    return {"properties": {"status": status}}
                if args[1] == "show":
                    return {"properties": {"latestRevisionName": "new"}}
            env = {"TF_VAR_backend_image": "registry.test/backend@sha256:" + "a"*64,
                   "TF_VAR_web_image": "registry.test/web@sha256:" + "b"*64,
                   "TF_VAR_service_version": "sha", "GITHUB_SHA": "sha", "GITHUB_RUN_ID": "1"}
            cwd = Path.cwd()
            try:
                with patch.object(release.Path, "resolve", return_value=root / "infra/scripts/release.py"), patch.object(release, "run", side_effect=command), patch.object(release, "az", side_effect=azure), patch.object(release.subprocess, "check_output", side_effect=console), patch.object(release.time, "sleep"), patch.dict(os.environ, env):
                    try:
                        release.release("test")
                    except (RuntimeError, TimeoutError, ValueError) as error:
                        return calls, error
                    return calls, None
            finally:
                os.chdir(cwd)

    def test_migration_failure_never_activates_new_revision(self):
        for status in ("Failed", "Canceled", "Running", "Stopped"):
            calls, error = self.scenario(status)
            self.assertIsNotNone(error)
            self.assertFalse(any("release.tfplan" in c or "activate" in c or "./infra/scripts/smoke-test.sh" in c for c in calls))

    def test_drain_timeout_blocks_migration(self):
        calls, error = self.scenario(replicas=[{"name": "busy"}])
        self.assertIsInstance(error, TimeoutError)
        self.assertFalse(any("plan" in c or "start" in c for c in calls))

    def test_retry_waits_for_already_inactive_revision(self):
        calls, error = self.scenario(replicas=[{"name": "still-draining"}], active=False)
        self.assertIsInstance(error, TimeoutError)
        self.assertFalse(any("plan" in c or "start" in c or "deactivate" in c for c in calls))

    def test_wrong_remote_state_blocks_before_stopping_apps(self):
        calls, error = self.scenario(state_environment="production")
        self.assertIsInstance(error, ValueError)
        self.assertFalse(any("deactivate" in c for c in calls))

    def test_seed_policy_blocks_before_stopping_apps(self):
        calls, error = self.scenario(seed=False)
        self.assertIsInstance(error, ValueError)
        self.assertFalse(any("deactivate" in c for c in calls))

    def test_success_order_and_explicit_digest_overrides(self):
        calls, error = self.scenario()
        self.assertIsNone(error)
        migration = next(i for i,c in enumerate(calls) if c[:4] == ("containerapp", "job", "execution", "show"))
        apply = next(i for i,c in enumerate(calls) if "apply" in c and "release.tfplan" in c)
        activate = next(i for i,c in enumerate(calls) if "activate" in c)
        smoke = next(i for i,c in enumerate(calls) if "./infra/scripts/smoke-test.sh" in c)
        self.assertLess(migration, apply)
        self.assertLess(apply, activate)
        self.assertLess(activate, smoke)
        for c in calls:
            if "plan" in c:
                self.assertTrue(any(x.startswith("-var=backend_image=") for x in c))

class PromotionTests(unittest.TestCase):
    def test_gate_rejects_missing_self_approval_and_admin_bypass(self):
        gate = {"protection_rules": [{"type": "required_reviewers", "reviewers": [{"id": 1}], "prevent_self_review": True}], "can_admins_bypass": False, "deployment_branch_policy": {"protected_branches": False, "custom_branch_policies": True}}
        dev = [{"name": "dev", "type": "branch"}]
        promotion.validate_gate(gate, dev, "dev")
        for invalid in ({}, {**gate, "can_admins_bypass": True}, {**gate, "protection_rules": []},
                        {**gate, "deployment_branch_policy": {"protected_branches": True, "custom_branch_policies": False}}):
            with self.assertRaises(ValueError):
                promotion.validate_gate(invalid, dev, "dev")
        for policies in ([], [{"name": "main", "type": "branch"}], dev + [{"name": "qa", "type": "branch"}], [{"name": "dev", "type": "tag"}]):
            with self.assertRaises(ValueError):
                promotion.validate_gate(gate, policies, "dev")
        gate["protection_rules"][0]["prevent_self_review"] = False
        with self.assertRaises(ValueError):
            promotion.validate_gate(gate, dev, "dev")

    def test_only_successful_matching_test_release_can_promote(self):
        run = {"id": 1, "conclusion": "success", "event": "push", "head_branch": "qa", "head_sha": "abc", "path": ".github/workflows/deploy.yml", "repository": {"full_name": "owner/repo"}}
        record = {"environment": "test", "smoke": "passed", "sha": "abc", "version": "abc", "run_id": "1", "backend_image": "r.test/backend@sha256:"+"a"*64, "web_image": "r.test/web@sha256:"+"b"*64}
        promotion.validate_release(run, record, "owner/repo", "1")
        for key, value in (("conclusion", "failure"), ("event", "pull_request"), ("head_sha", "other"), ("head_branch", "feature"), ("head_branch", "dev"), ("head_branch", "main")):
            with self.assertRaises(ValueError):
                promotion.validate_release({**run, key: value}, record, "owner/repo", "1")
        with self.assertRaises(ValueError):
            promotion.validate_release(run, {**record, "backend_image": "r.test/backend:latest"}, "owner/repo", "1")
