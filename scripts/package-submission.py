#!/usr/bin/env python3
"""Create the assessment ZIP only from reviewed source and supplied final material."""
import argparse
import json
from pathlib import Path
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_PARTS = {".git", ".terraform", "node_modules", "target", ".next", "__pycache__", ".aws", ".codex", ".agents", ".serena"}
SOURCE_ROOTS = {"backend", "frontend", "infra", ".github", "scripts"}


def source_allowed(path):
    p = Path(path)
    return (not any(part in EXCLUDED_PARTS for part in p.parts)
            and not any(part.startswith(".env") and part != ".env.example" for part in p.parts)
            and not p.name.endswith((".tfstate", ".tfplan", ".tfvars", ".tfvars.json", ".backend.hcl", ".pyc", ".tsbuildinfo", ".log"))
            and ".tfstate." not in p.name
            and p.name != "next-env.d.ts"
            and not (path == "infra/bootstrap/backend.tf"))


def package(output, name, user_id, report, media):
    if not name.strip() or not user_id.strip() or not report.is_file():
        raise ValueError("Supply the student's name, user ID and final report")
    files = sorted(media.iterdir())
    if not files or any(not f.is_file() or f.suffix.lower() not in {".png", ".jpg", ".jpeg", ".mp4"} for f in files):
        raise ValueError("Media must contain only PNG/JPG images and MP4 clips")
    duration = 0.0
    clips = 0
    for file in files:
        if file.suffix.lower() == ".mp4":
            data = json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(file)], text=True))
            videos = [s for s in data["streams"] if s["codec_type"] == "video"]
            length = float(data["format"]["duration"])
            if not videos or any(s["codec_name"] != "h264" for s in videos) or not 20 <= length <= 30:
                raise ValueError(f"Clip must use H.264 and last 20–30 seconds: {file.name}")
            duration += length
            clips += 1
    if not clips or duration >= 300:
        raise ValueError("Supply H.264 clips with total duration under five minutes")
    # Include new task files too; ignored overrides, state and builds never enter the list.
    paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=ROOT).decode().split("\0")
    with zipfile.ZipFile(output, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("readme.txt", f"Campus Device Loan System\nName: {name}\nUser ID: {user_id}\nSee Documentation for results and remaining limits.\n")
        for path in sorted(set(paths)):
            if not path or not source_allowed(path) or not (ROOT / path).is_file() or (ROOT / path).is_symlink():
                continue
            first = Path(path).parts[0]
            if first in SOURCE_ROOTS or path in {"Makefile", ".gitignore", "README.md"}:
                archive.write(ROOT / path, "Source/" + path)
            elif first == "docs":
                archive.write(ROOT / path, "Documentation/" + str(Path(path).relative_to("docs")))
        archive.write(report, "Documentation/FinalReport" + report.suffix.lower())
        for file in files:
            archive.write(file, "Media/" + file.name)
    print(json.dumps({"zip": str(output), "clips": clips, "duration_seconds": duration, "status": "packaged; student must review content before submission"}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--name", required=True)
    parser.add_argument("--user-id", required=True)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--media", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    package(args.output, args.name, args.user_id, args.report, args.media)
