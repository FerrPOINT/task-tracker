#!/usr/bin/env python3
"""Regression checks for Task Tracker API contract gates in CI."""

from __future__ import annotations

import unittest
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/ci.yml"


class CiContractChecksTest(unittest.TestCase):
    def test_full_release_delegates_to_the_original_manual_http_capable_cohort(self) -> None:
        release = (ROOT / '.github/workflows/release.yml').read_text(encoding='utf-8')
        qualification = (ROOT / '.github/workflows/qualification.yml').read_text(encoding='utf-8')
        self.assertIn('uses: ./.github/workflows/qualification.yml', release)
        self.assertIn('http_only: false', release)
        self.assertNotIn('concurrency:', release)
        self.assertIn('workflow_call:', qualification)
        self.assertIn('workflow_dispatch:', qualification)
        self.assertIn('http_only:', qualification)
        self.assertIn('concurrency:', qualification)

    def test_frontend_uses_ui_pin_and_backend_retains_sdk_pin(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        frontend = re.split(r"\n  [a-zA-Z0-9_-]+:\n", workflow.split("\n  frontend:\n", 1)[1], 1)[0]
        backend = workflow.split("\n  backend:\n", 1)[1].split("\n  frontend:\n", 1)[0]
        self.assertIn("pin=.base-ui-revision", frontend)
        self.assertNotIn("pin=.namespace-base-revision", frontend)
        self.assertIn("pin=.base-revision", backend)
        self.assertIn("pin=.namespace-base-revision", backend)

    def test_deleted_key_restore_uses_dedicated_postgres_database(self) -> None:
        workflow = (ROOT / ".github/workflows/qualification.yml").read_text(encoding="utf-8")
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


    def test_backend_explicitly_runs_disposable_postgres_sdlc_tests(self) -> None:
        workflow = (ROOT / ".github/workflows/qualification.yml").read_text(encoding="utf-8")
        backend_job = workflow.split("\n  backend:\n", 1)[1].split("\n  frontend:\n", 1)[0]
        self.assertIn("image: postgres:17.6", backend_job)
        self.assertIn("TT_SDLC_DRAFT_TEST_DATABASE_URL:", backend_job)
        self.assertIn("TT_SDLC_TEST_DATABASE_URL:", backend_job)
        self.assertIn("createdb -U tasktracker tracker_drafts", backend_job)
        self.assertIn("createdb -U tasktracker tracker_clarifications", backend_job)
        for suite in ("drafts", "sdlc"):
            self.assertIn(
                f"cargo test -p server --test {suite} --locked -- --ignored --test-threads=1",
                backend_job,
            )


    def test_frontend_job_checks_openapi_backward_compatibility(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        frontend_job = workflow.split("\n  frontend:\n", 1)[1]
        self.assertIn("fetch-depth: 0", frontend_job)
        self.assertIn("pnpm openapi:check", frontend_job)
        self.assertIn("pnpm openapi:compat", frontend_job)
        self.assertIn("pnpm format:check", frontend_job)

    def test_configured_pm_gate_executes_both_current_authority_cases(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertNotIn("unsubmitted_recovery_tests::", workflow)
        for name in (
            "owner_can_adopt_only_an_unstarted_draft_without_rewriting_its_creation",
            "configured_cli_business_writes_need_current_assignment_but_no_native_lease",
        ):
            self.assertIn(name, workflow)
        self.assertIn('sdlc::candidates::tests::configured_cli_tests::$case_name', workflow)
        self.assertIn('dropdb -U tasktracker tracker_pm_candidates_test', workflow)

    def test_configured_start_gate_executes_each_case_in_a_fresh_database(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        for name in (
            "forward_start_migration_preserves_filled_110_history_without_auto_start",
            "explicit_start_is_atomic_discovered_once_and_replays_original_after_advancement",
            "start_refuses_nonowners_no_opt_in_missing_consent_stale_and_foreign_receipts",
            "start_race_payload_conflict_and_current_access_readback_do_not_duplicate_effects",
            "start_sql_rejects_detached_command_and_direct_permit_bypass",
        ):
            self.assertIn(name, workflow)
        self.assertIn(
            'sdlc::candidates::tests::critical_blocker_tests::configured_start_tests::$case_name',
            workflow,
        )
        self.assertIn("createdb -U tasktracker tracker_start_candidates_test", workflow)
        self.assertIn("dropdb -U tasktracker tracker_start_candidates_test", workflow)
        self.assertIn("--exact --ignored --test-threads=1", workflow)
        self.assertIn("test result: ok. 1 passed; 0 failed; 0 ignored;", workflow)

    def test_populated_upgrade_runs_exactly_one_ignored_test_in_fresh_database(self) -> None:
        workflow = (ROOT / ".github/workflows/qualification.yml").read_text(encoding="utf-8")
        backend = workflow.split("\n  backend:\n", 1)[1].split("\n  frontend:\n", 1)[0]
        self.assertIn("createdb -U tasktracker tracker_upgrade_draft_flow_test", backend)
        self.assertIn(
            "TT_DRAFT_FLOW_TEST_DATABASE_URL: postgres://tasktracker@127.0.0.1:5432/tracker_upgrade_draft_flow_test",
            backend,
        )
        self.assertIn(
            "cargo test --locked -p infra --lib populated_0033_through_095_to_096_preserves_original_ids_hashes_and_history -- --ignored --test-threads=1",
            backend,
        )
        self.assertIn("test result: ok. 1 passed; 0 failed; 0 ignored;", backend)



if __name__ == "__main__":
    unittest.main()
