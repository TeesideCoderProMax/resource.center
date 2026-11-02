#!/usr/bin/env python3
"""Verify GitHub gate settings and the successful Test run before Azure login."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def validate_gate(gate):
    rules = gate.get("protection_rules", [])
    reviewers = next((r for r in rules if r.get("type") == "required_reviewers"), {})
    branches = gate.get("deployment_branch_policy") or {}
    if not (reviewers.get("reviewers") and reviewers.get("prevent_self_review")
            and gate.get("can_admins_bypass") is False
            and branches.get("protected_branches")):
        raise ValueError("Promotion requires independent reviewers, no admin bypass and protected branches")


def validate_release(run, release, repository, run_id):
    if not (run.get("conclusion") == "success" and run.get("event") == "push"
            and run.get("head_branch") == "main" and run.get("path") == ".github/workflows/deploy.yml"
            and run.get("repository", {}).get("full_name") == repository
            and str(run.get("id")) == str(run_id)):
        raise ValueError("Source must be a successful main push release in this repository")
    if not (release.get("environment") == "test" and release.get("smoke") == "passed"
            and release.get("sha") == run.get("head_sha") and str(release.get("run_id")) == str(run_id)
            and release.get("version") == run.get("head_sha")):
        raise ValueError("Test release identity mismatch")
    for key in ("backend_image", "web_image"):
        if not re.fullmatch(r"[a-z0-9.-]+/[a-z0-9/_-]+@sha256:[0-9a-f]{64}", release.get(key, "")):
            raise ValueError("Invalid image digest")


def main(environment, run_id):
    if environment not in {"staging", "production"} or not run_id.isdecimal():
        raise ValueError("Expected target environment and numeric Test run ID")
    repo = os.environ["GITHUB_REPOSITORY"]
    def api(path):
        return json.loads(subprocess.check_output(["gh", "api", f"repos/{repo}/{path}"], text=True))
    validate_gate(api(f"environments/{environment}"))
    run = api(f"actions/runs/{run_id}")
    subprocess.run(["gh", "run", "download", run_id, "--repo", repo, "--name", "test-release", "--dir", "tested-release"], check=True)
    release = json.loads(Path("tested-release/release.json").read_text())
    validate_release(run, release, repo, run_id)
    with open(os.environ["GITHUB_ENV"], "a") as f:
        for key in ("backend_image", "web_image"):
            f.write(f"TF_VAR_{key}={release[key]}\n")
        f.write(f"TF_VAR_service_version={release['version']}\n")


if __name__ == "__main__":
    main(*sys.argv[1:])
