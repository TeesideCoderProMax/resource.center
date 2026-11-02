#!/usr/bin/env python3
"""Fail-closed, maintenance-window release. No schema downgrade or auto rollback."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def az(*args):
    return json.loads(run("az", *args, "-o", "json") or "null")


def wait_for(read, done, failed, attempts=90):
    for _ in range(attempts):
        value = read()
        if done(value):
            return value
        if failed(value):
            raise RuntimeError(f"Release step failed: {value}")
        time.sleep(5)
    raise TimeoutError("Release step timed out; apps remain stopped")


def release(environment):
    if environment not in {"test", "staging", "production"}:
        raise ValueError("Unknown environment")
    root = Path(__file__).resolve().parents[2]
    os.chdir(root)
    config = root / "infra/environments" / environment
    # Reviewed real environment configuration is required; never copy example handoffs.
    for name in ("workloads.tfvars", "workloads.backend.hcl"):
        if not (config / name).is_file():
            raise ValueError(f"Missing reviewed {config / name}")
    for key in ("TF_VAR_backend_image", "TF_VAR_web_image"):
        if not re.fullmatch(r"[a-z0-9.-]+/[a-z0-9/_-]+@sha256:[0-9a-f]{64}", os.environ[key]):
            raise ValueError(f"Invalid immutable image: {key}")
    os.environ["TF_DATA_DIR"] = str(root / "infra/workloads/.terraform" / environment)
    run("./infra/scripts/terraform.sh", "init", "workloads", environment)
    tf = ("terraform", "-chdir=infra/workloads")
    state = json.loads(run(*tf, "show", "-json"))
    existing_environment = state.get("values", {}).get("outputs", {}).get("environment", {}).get("value")
    if existing_environment and existing_environment != environment:
        raise ValueError("Remote state belongs to another environment")
    # Read desired values through Terraform, including first-release job identity.
    varargs = (f"-var-file={config / 'workloads.tfvars'}", f"-var=environment={environment}",
               *(f"-var={key}={os.environ['TF_VAR_' + key]}" for key in ("backend_image", "web_image", "service_version")))
    def value(expression):
        # Newline-terminated so console evaluates the line; jsonencode gives exact JSON.
        result = subprocess.run((*tf, "console", "-no-color", *varargs), input=f"jsonencode({expression})\n",
                                text=True, capture_output=True)
        out = result.stdout.strip()
        if result.returncode or not out:
            raise RuntimeError(f"terraform console failed for {expression}: {result.stderr.strip() or 'no output'}")
        # Console prints the encoded string as an HCL string literal.
        return json.loads(json.loads(out))
    group = value("var.handoff.workload_resource_group")
    prefix = value("var.name_prefix")
    if value("var.handoff.environment") != environment:
        raise ValueError("Foundation environment mismatch")
    if not value("var.seed_demo_data"):
        raise ValueError("Assessment releases require safe demo seeding in each environment")
    apps = az("containerapp", "list", "-g", group)
    stopped = []
    # Stop old API writers and old workers before incompatible schema/lease rollout.
    # Listing must succeed: an API/access failure must not look like an empty first release.
    for app in apps:
        if app["name"] not in {prefix + "-loan-api", prefix + "-notifications", prefix + "-web"}:
            continue
        for revision in az("containerapp", "revision", "list", "-g", group, "-n", app["name"]):
            name = revision["name"]
            if revision["properties"]["active"]:
                az("containerapp", "revision", "deactivate", "-g", group, "-n", app["name"], "--revision", name)
            # An interrupted release can leave an inactive revision still draining.
            stopped.append((app["name"], name))
    for app, revision in stopped:
        wait_for(lambda: az("containerapp", "replica", "list", "-g", group, "-n", app, "--revision", revision),
                 lambda rows: len(rows) == 0, lambda _: False)
    # Target only the migration job. This also supports first release after images exist.
    # Terraform remains owner; no competing CLI image updates.
    run(*tf, "plan", "-input=false", "-lock-timeout=60s", *varargs,
        "-target=azurerm_container_app_job.migrations", "-out=migration.tfplan")
    run(*tf, "apply", "-input=false", "-lock-timeout=60s", "migration.tfplan")
    job = prefix + "-migrations"
    execution = az("containerapp", "job", "start", "-g", group, "-n", job)["name"]
    print(json.dumps({"stage": "migration_started", "execution": execution}), flush=True)
    started = time.monotonic()
    wait_for(lambda: az("containerapp", "job", "execution", "show", "-g", group, "-n", job,
                        "--job-execution-name", execution)["properties"]["status"],
             lambda status: status == "Succeeded", lambda status: status in {"Failed", "Canceled", "Stopped"})
    print(json.dumps({"migration_execution": execution, "duration_seconds": round(time.monotonic()-started, 2)}))
    # No application plan/apply can happen until migration AND seed transaction commits.
    run(*tf, "plan", "-input=false", "-lock-timeout=60s", *varargs, "-out=release.tfplan")
    run(*tf, "apply", "-input=false", "-lock-timeout=60s", "release.tfplan")
    for suffix in ("loan-api", "notifications", "web"):
        app = prefix + "-" + suffix
        latest = az("containerapp", "show", "-g", group, "-n", app)["properties"]["latestRevisionName"]
        az("containerapp", "revision", "activate", "-g", group, "-n", app, "--revision", latest)
    outputs = json.loads(run(*tf, "output", "-json"))
    os.environ["LOAN_API_URL"] = outputs["loan_api_url"]["value"]
    os.environ["WEB_URL"] = outputs["web_app_url"]["value"]
    os.environ["EXPECTED_VERSION"] = os.environ["TF_VAR_service_version"]
    run("./infra/scripts/smoke-test.sh")
    record = {"environment": environment, "sha": os.environ["TF_VAR_service_version"],
              "workflow_sha": os.environ["GITHUB_SHA"],
              "run_id": os.environ["GITHUB_RUN_ID"], "version": os.environ["TF_VAR_service_version"],
              "backend_image": os.environ["TF_VAR_backend_image"], "web_image": os.environ["TF_VAR_web_image"],
              "migration_execution": execution, "smoke": "passed"}
    Path("release.json").write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record))


if __name__ == "__main__":
    release(sys.argv[1])
