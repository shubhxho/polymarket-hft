#!/usr/bin/env python3
"""Stage only fresh CI evidence, with byte hashes and commit provenance."""
import hashlib
import json
import os
from pathlib import Path
import re
import sys

NAMES = ("checks.txt", "run-through.txt", "benchmark.json", "stream-benchmark.json", "test-inventory.txt")

def stage(source, destination, commit, run_id):
    if not re.fullmatch(r"[0-9a-f]{40}", commit) or not re.fullmatch(r"[0-9]+", run_id):
        raise ValueError("invalid CI provenance")
    source, destination = Path(source), Path(destination)
    if destination.exists():
        raise ValueError("artifact destination must be new")
    records = []
    for name in NAMES:
        path = source / name
        if path.is_symlink() or not path.is_file():
            raise ValueError("missing or linked CI evidence: " + name)
        if path.stat().st_size > 16 * 1024 * 1024:
            raise ValueError("CI evidence too large")
        data = path.read_bytes()
        if not data:
            raise ValueError("empty CI evidence")
        if name.endswith(".json"):
            json.loads(data)
        records.append((name, data))
    destination.mkdir(parents=True)
    manifest = {"schema_version": 1, "commit": commit, "run_id": run_id,
                "profile": "compact", "gpu_validated": False, "paper_only": True,
                "files": []}
    for name, data in records:
        (destination / name).write_bytes(data)
        manifest["files"].append({"path": name, "bytes": len(data),
                                  "sha256": hashlib.sha256(data).hexdigest()})
    (destination / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest

if __name__ == "__main__":
    stage(sys.argv[1], sys.argv[2], os.environ["GITHUB_SHA"], os.environ["GITHUB_RUN_ID"])
