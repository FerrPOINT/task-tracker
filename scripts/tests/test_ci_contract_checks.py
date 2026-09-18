#!/usr/bin/env python3
"""Regression checks for Task Tracker API contract gates in CI."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/ci.yml"


class CiContractChecksTest(unittest.TestCase):
    def test_frontend_job_checks_openapi_backward_compatibility(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        frontend_job = workflow.split("\n  frontend:\n", 1)[1]
        self.assertIn("fetch-depth: 0", frontend_job)
        self.assertIn("pnpm openapi:check", frontend_job)
        self.assertIn("pnpm openapi:compat", frontend_job)
        self.assertIn("pnpm format:check", frontend_job)


if __name__ == "__main__":
    unittest.main()
