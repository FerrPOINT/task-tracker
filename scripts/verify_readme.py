#!/usr/bin/env python3
"""Validate Task Tracker README structure and reviewed public evidence."""

from __future__ import annotations

import re
import sys
from pathlib import Path
from urllib.parse import unquote

REQUIRED_ANCHORS = {
    "overview",
    "capabilities",
    "quick-start",
    "visual-proof",
    "safety",
    "quality",
    "license",
}
REQUIRED_PROOF = {
    "10-reports.png",
    "m-board-viewport.png",
}
MD_IMAGE_RE = re.compile(r"!\[[^]]*\]\(([^)]+)\)")
HTML_IMAGE_RE = re.compile(r"<img\b[^>]*\bsrc=[\"']([^\"']+)[\"']", re.IGNORECASE)
ANCHOR_RE = re.compile(r"<a\b[^>]*\bname=[\"']([^\"']+)[\"']", re.IGNORECASE)
WORKFLOW_RE = re.compile(r"actions/workflows/([^/?#]+\.ya?ml)", re.IGNORECASE)
LOCAL_PATH_RE = re.compile(r"(?:^|[\s`])/(?:opt|home)/(?:dev|root)(?:/|\b)")


def validate(root: Path) -> list[str]:
    readme = root / "README.md"
    if not readme.is_file():
        return ["RMD001: README.md: missing file"]

    text = readme.read_text(encoding="utf-8")
    findings: list[str] = []
    anchors = set(ANCHOR_RE.findall(text))
    for anchor in sorted(REQUIRED_ANCHORS - anchors):
        findings.append(f"RMD002: README.md: missing anchor: {anchor}")

    if "{{" in text or LOCAL_PATH_RE.search(text):
        findings.append("RMD003: README.md: placeholder or local filesystem path")

    proof = set(re.findall(r"docs/screenshots/([^)]+\.png)", text))
    for name in sorted(REQUIRED_PROOF - proof):
        findings.append(f"RMD004: README.md: missing proof asset: {name}")

    for image in MD_IMAGE_RE.findall(text) + HTML_IMAGE_RE.findall(text):
        image = unquote(image)
        if image.startswith(("http://", "https://", "data:", "#")):
            continue
        if not (root / image).is_file():
            findings.append(f"RMD005: README.md: missing image: {image}")

    for workflow in sorted(set(WORKFLOW_RE.findall(text))):
        if not (root / ".github/workflows" / workflow).is_file():
            findings.append(f"RMD006: README.md: missing workflow badge target: {workflow}")

    return findings


def main() -> int:
    findings = validate(Path(__file__).resolve().parents[1])
    if findings:
        print("\n".join(findings), file=sys.stderr)
        return 1
    print("README validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
