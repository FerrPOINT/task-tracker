#!/usr/bin/env python3
"""Regression checks for Task Tracker API contract gates in CI."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/ci.yml"


class CiContractChecksTest(unittest.TestCase):
    def test_deleted_key_restore_uses_dedicated_postgres_database(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        backend = workflow.split("\n  backend:\n", 1)[1].split("\n  frontend:\n", 1)[0]
        self.assertIn("image: postgres:17.6-alpine", backend)
        self.assertIn("POSTGRES_DB: tasktracker_infra_test", backend)
        self.assertIn(
            "TT_TEST_DATABASE_URL: postgres://tasktracker@127.0.0.1:5432/tasktracker_infra_test",
            backend,
        )
        self.assertIn(
            "cargo test --locked -p infra --test repos deleted_issue_key_can_be_resolved_for_restore_only -- --ignored --test-threads=1",
            backend,
        )
        self.assertIn("cargo test --locked --workspace -- --test-threads=1", backend)

    def test_frontend_job_checks_openapi_backward_compatibility(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        frontend_job = workflow.split("\n  frontend:\n", 1)[1]
        self.assertIn("fetch-depth: 0", frontend_job)
        self.assertIn("pnpm openapi:check", frontend_job)
        self.assertIn("pnpm openapi:compat", frontend_job)
        self.assertIn("pnpm format:check", frontend_job)


if __name__ == "__main__":
    unittest.main()
