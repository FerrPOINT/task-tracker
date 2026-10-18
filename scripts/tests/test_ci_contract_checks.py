#!/usr/bin/env python3
"""Regression checks for Task Tracker API contract gates in CI."""

from __future__ import annotations

import importlib.util
import re
import shlex
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/release.yml"


class CiContractChecksTest(unittest.TestCase):
    def test_normal_seaorm_consumers_do_not_enable_the_mock_driver(self) -> None:
        workspace = tomllib.loads((ROOT / 'backend/Cargo.toml').read_text(encoding='utf-8'))['workspace']
        seaorm = workspace['dependencies']['sea-orm']
        self.assertIs(seaorm['default-features'], False)
        self.assertNotIn('mock', seaorm['features'])
        self.assertTrue({'sqlx-postgres', 'runtime-tokio-rustls', 'macros', 'with-json',
                         'with-uuid', 'with-chrono'}.issubset(seaorm['features']))
        for member in workspace['members']:
            manifest = tomllib.loads((ROOT / 'backend' / member / 'Cargo.toml').read_text(encoding='utf-8'))
            tables = [manifest, *manifest.get('target', {}).values()]
            for table in tables:
                for kind in ('dependencies', 'build-dependencies'):
                    for alias, dependency in table.get(kind, {}).items():
                        if alias == 'sea-orm' or (isinstance(dependency, dict) and dependency.get('package') == 'sea-orm'):
                            with self.subTest(member=member, kind=kind, alias=alias):
                                self.assertIsInstance(dependency, dict)
                                self.assertIs(dependency.get('workspace'), True)
                                self.assertNotIn('mock', dependency.get('features', []))
            for values in manifest.get('features', {}).values():
                self.assertNotIn('sea-orm/mock', values)

    def test_infra_mock_driver_is_owned_by_its_existing_dev_dependency(self) -> None:
        infra = tomllib.loads((ROOT / 'backend/infra/Cargo.toml').read_text(encoding='utf-8'))
        dependency = infra['dev-dependencies'].get('sea-orm', {})
        self.assertIs(dependency.get('workspace'), True)
        self.assertEqual(dependency.get('features'), ['mock'])
        self.assertNotIn('version', dependency, 'The test driver must keep the same locked SeaORM package')

    def test_contract_test_edits_select_fast_tooling_without_application_builds(self) -> None:
        spec = importlib.util.spec_from_file_location(
            'tracker_quality_selection', ROOT / 'scripts/vendor/base_quality_selection.py')
        selector = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(selector)
        changed = selector.plan(ROOT, profile='build', changed=['scripts/tests/test_ci_contract_checks.py'])
        self.assertEqual([gate['id'] for gate in changed['gates']], ['tooling:ci-contracts'])
        self.assertEqual(changed['gates'][0]['execution_slot'], 'none')
        production = selector.plan(ROOT, profile='build', changed=['backend/Cargo.toml'])
        self.assertTrue(any(gate['id'] == 'backend:build' for gate in production['gates']))
        self.assertTrue(any(gate['id'] == 'tooling:ci-contracts' for gate in production['gates']))

    def test_ordinary_ci_builds_only_runtime_and_contract_targets(self) -> None:
        workflow = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        backend = workflow.split('\n  backend:\n', 1)[1].split('\n  frontend:\n', 1)[0]
        build = backend.split('- name: Build changed application after isolated units', 1)[1].split('- name:', 1)[0]
        command = next(line.strip().removeprefix('run: ') for line in build.splitlines() if 'run: cargo build' in line)
        self.assertEqual(shlex.split(command), [
            'cargo', 'build', '--locked', '-p', 'server', '--bin', 'server', '-p', 'api', '--bin', 'gen-openapi',
        ])
        condition = "if: needs.changes.outputs.backend_build == 'true'"
        self.assertIn(condition, build)
        contract = backend.split('- name: OpenAPI drift gate', 1)[1]
        self.assertIn(condition, contract)
        self.assertIn('debug/gen-openapi', contract)
        release = WORKFLOW.read_text(encoding='utf-8')
        self.assertIn('cargo test --locked --workspace -- --test-threads=1', release)

    def test_automatic_units_do_not_prepare_postgres_and_http_mode_stays_manual(self) -> None:
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        backend = workflow.split("\n  backend:\n", 1)[1].split("\n  frontend:\n", 1)[0]
        http = workflow.split("\n  http:\n", 1)[1]
        self.assertNotIn("services:", backend)
        self.assertNotIn("--ignored", backend)
        self.assertIn("cargo test --locked -p domain --lib", backend)
        self.assertIn("github.event_name == 'workflow_dispatch' && inputs.http_only", http)
        self.assertIn("services:", http)
        self.assertIn("cargo clippy --locked -p server --test drafts --test sdlc -- -D warnings", http)
        for suite in ("drafts", "sdlc"):
            self.assertIn(f"cargo test -p server --test {suite} --locked -- --ignored --test-threads=1", http)
        self.assertNotIn("populated_0033_through_095", http)
        self.assertEqual(http.count("run: python3 ../scripts/vendor/base_rust_target_cache.py"), 1)

    def test_frontend_uses_ui_pin_and_backend_retains_sdk_pin(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        frontend = re.split(r"\n  [a-zA-Z0-9_-]+:\n", workflow.split("\n  frontend:\n", 1)[1], maxsplit=1)[0]
        backend = workflow.split("\n  backend:\n", 1)[1].split("\n  frontend:\n", 1)[0]
        self.assertIn("pin=.base-ui-revision", frontend)
        self.assertNotIn("pin=.namespace-base-revision", frontend)
        self.assertIn("pin=.base-revision", backend)
        self.assertIn("pin=.namespace-base-revision", backend)

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

    def test_backend_explicitly_runs_disposable_postgres_sdlc_tests(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
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
        self.assertIn("pnpm install --frozen-lockfile --ignore-scripts=false", frontend_job)
        self.assertIn("git diff --exit-code -- pnpm-lock.yaml src/api/generated.ts", frontend_job)
        self.assertIn("pnpm openapi:compat", frontend_job)
        self.assertIn("pnpm format:check", frontend_job)

    def test_populated_upgrade_runs_exactly_one_ignored_test_in_fresh_database(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
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
