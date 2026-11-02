import importlib.util
from pathlib import Path
import unittest
import json
import tempfile
import zipfile
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("package", Path(__file__).resolve().parents[2] / "scripts/package-submission.py")
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)

class PackageTests(unittest.TestCase):
    def test_excludes_local_state_secrets_and_builds(self):
        for path in ("frontend/.env.local", "infra/workloads/test.tfstate", "infra/workloads/test.tfstate.backup", "infra/workloads/test.tfplan", "infra/environments/test/workloads.tfvars", "frontend/node_modules/pkg/index.js", "backend/target/release/loan-service", "infra/scripts/__pycache__/x.pyc"):
            self.assertFalse(package.source_allowed(path), path)
        for path in ("backend/Cargo.lock", "frontend/package-lock.json", "infra/environments/test/workloads.tfvars.example", "infra/workloads/.terraform.lock.hcl", ".github/workflows/ci.yml"):
            self.assertTrue(package.source_allowed(path), path)

    def test_zip_layout_and_media_policy_with_synthetic_inputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "backend").mkdir()
            (root / "backend/main.rs").write_text("// synthetic source")
            (root / "backend/.env").write_text("excluded fixture")
            (root / "docs").mkdir()
            (root / "docs/result.md").write_text("Synthetic test result")
            report = root / "report.md"
            report.write_text("Synthetic report; not submission content")
            media = root / "media"
            media.mkdir()
            (media / "clip.mp4").write_bytes(b"synthetic; ffprobe is mocked")
            output = root / "submission.zip"
            def command(args, **kwargs):
                if args[0] == "ffprobe":
                    return json.dumps({"streams": [{"codec_type": "video", "codec_name": "h264"}], "format": {"duration": "25"}})
                return b"backend/main.rs\0backend/.env\0docs/result.md\0"
            with patch.object(package, "ROOT", root), patch.object(package.subprocess, "check_output", side_effect=command):
                package.package(output, "Synthetic Student", "TEST-ID", report, media)
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(set(archive.namelist()), {"readme.txt", "Source/backend/main.rs", "Documentation/result.md", "Documentation/FinalReport.md", "Media/clip.mp4"})
                self.assertIsNone(archive.testzip())
            with self.assertRaises(ValueError):
                package.package(root / "missing.zip", "", "TEST-ID", report, media)
            self.assertFalse((root / "missing.zip").exists())
