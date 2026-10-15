#!/usr/bin/env python3
"""Compatibility entrypoint for the immutable Base selector package."""
from __future__ import annotations
import hashlib
import importlib.util
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def classify(event_name: str, base_sha: str, head_sha: str, *, repo: Path = ROOT) -> dict[str, str]:
    artifact = ROOT / "scripts/vendor/base_quality_selection.py"
    receipt = json.loads((artifact.parent / "base-quality-tooling.json").read_text(encoding="utf-8"))
    pin = (ROOT / ".quality-tooling-revision").read_text(encoding="utf-8").strip()
    if receipt["revision"] != pin or hashlib.sha256(artifact.read_bytes()).hexdigest() != receipt["sha256"]:
        raise ValueError("Packaged Base classifier identity mismatch")
    spec = importlib.util.spec_from_file_location("base_quality_selector", artifact)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.classify(event_name, base_sha, head_sha, repo=repo, manifest_root=ROOT)


def main() -> None:
    result = classify(os.environ["EVENT_NAME"], os.environ.get("BASE_SHA", ""), os.environ.get("HEAD_SHA", ""))
    output = "".join(f"{name}={value}\n" for name, value in result.items())
    if os.environ.get("GITHUB_OUTPUT"):
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8", newline="") as stream:
            stream.write(output)
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
