#!/usr/bin/env python3
"""Classify PR paths using the source head, not GitHub's synthetic merge ref."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SHARED_PATHS = (
    "openapi/",
    ".base-revision",
    ".namespace-base-revision",
    ".github/workflows/",
    "scripts/classify_ci_changes.py",
)
BACKEND_PATHS = ("backend/",) + SHARED_PATHS
FRONTEND_PATHS = ("frontend/",) + SHARED_PATHS


def classify(event_name: str, base_sha: str, head_sha: str, *, repo: Path = ROOT) -> dict[str, str]:
    if event_name != "pull_request":
        return {"backend": "true", "frontend": "true"}
    if not base_sha or not head_sha:
        raise ValueError("pull_request classification requires base and head SHAs")

    changed = subprocess.run(
        ["git", "diff", "--name-only", f"{base_sha}...{head_sha}"],
        cwd=repo,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    return {
        "backend": str(any(path.startswith(BACKEND_PATHS) for path in changed)).lower(),
        "frontend": str(any(path.startswith(FRONTEND_PATHS) for path in changed)).lower(),
    }


def main() -> None:
    result = classify(
        os.environ["EVENT_NAME"],
        os.environ.get("BASE_SHA", ""),
        os.environ.get("HEAD_SHA", ""),
    )
    output = "".join(f"{name}={value}\n" for name, value in result.items())
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with Path(output_path).open("a", encoding="utf-8", newline="") as stream:
            stream.write(output)
    else:
        print(output, end="")


if __name__ == "__main__":
    main()
