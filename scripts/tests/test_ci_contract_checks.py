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




class ServerCliMetadataContractTest(unittest.TestCase):
    def test_server_keeps_production_sdk_sources_and_only_unselected_cli_metadata(self):
        import re
        import tomllib
        backend, base = ROOT / 'backend', ROOT.parent / 'services-base'
        workspace = tomllib.loads((backend / 'Cargo.toml').read_text(encoding='utf-8'))['workspace'].get('dependencies', {})
        pending, seen, required = [backend / 'server'], set(), set()
        while pending:
            directory = pending.pop().resolve()
            if directory in seen:
                continue
            seen.add(directory)
            manifest = tomllib.loads((directory / 'Cargo.toml').read_text(encoding='utf-8'))
            for table in [manifest, *manifest.get('target', {}).values()]:
                for kind in ('dependencies', 'build-dependencies'):
                    for name, dependency in table.get(kind, {}).items():
                        if not isinstance(dependency, dict):
                            continue
                        owner = directory
                        if dependency.get('workspace'):
                            dependency, owner = workspace[name], backend
                        if not isinstance(dependency, dict) or 'path' not in dependency:
                            continue
                        target = (owner / dependency['path']).resolve()
                        if target.is_relative_to(base):
                            required.add(target.relative_to(base).as_posix())
                        else:
                            self.assertTrue(target.is_relative_to(backend), 'Unaudited production path dependency')
                            pending.append(target)
        self.assertNotIn((backend / 'cli').resolve(), seen)
        cli = tomllib.loads((backend / 'cli/Cargo.toml').read_text(encoding='utf-8'))
        edges = [(backend / 'cli' / value['path']).resolve().relative_to(base).as_posix()
                 for value in cli['dependencies'].values() if isinstance(value, dict) and 'path' in value]
        self.assertEqual(len(edges), 1)
        sdk = edges[0]
        self.assertTrue(sdk.endswith('-cli-core'))
        self.assertNotIn(sdk, required)
        self.assertEqual(cli['bin'][0]['path'], 'src/main.rs')
        self.assertEqual(len(required), 3)
        for filename in ('Dockerfile', 'Dockerfile.umbrella'):
            with self.subTest(recipe=filename):
                text = (backend / filename).read_text(encoding='utf-8')
                copied = set(re.findall(r'^COPY services-base/((?:crates|base-modules/rust)/\S+)\s', text, re.M))
                self.assertEqual(copied, required | {sdk + '/Cargo.toml'})
                self.assertIn(f'COPY services-base/{sdk}/Cargo.toml /services-base/{sdk}/Cargo.toml', text)
                self.assertIn('COPY task-tracker/backend/cli/Cargo.toml cli/Cargo.toml', text)
                self.assertNotIn('COPY task-tracker/backend/cli ./cli', text)
                self.assertNotIn('cli/src', '\n'.join(line for line in text.splitlines() if 'rm -rf' in line))
                sdk_guard = f'compile_error!("metadata-only {Path(sdk).name} must not be compiled");'
                cli_guard = 'compile_error!("metadata-only task-tracker CLI must not be compiled");'
                self.assertIn(sdk_guard, text)
                self.assertIn(cli_guard, text)
                first_cargo = re.search(r'\bcargo (?:test|build) ', text).start()
                self.assertLess(text.index(sdk_guard), first_cargo)
                self.assertLess(text.index(cli_guard), first_cargo)
                builds = re.findall(r'\bcargo build ([^\n\\]+)', text)
                self.assertTrue(builds)
                self.assertTrue(all('--bin server' in value for value in builds))


class PnpmLifecyclePolicyTests(unittest.TestCase):
    def test_esbuild_has_one_explicit_dependency_build_policy(self):
        from pathlib import Path
        root = Path(__file__).resolve().parents[2]
        lines = (root / 'frontend/pnpm-workspace.yaml').read_text(encoding='utf-8').splitlines()
        self.assertEqual(lines.count('allowBuilds:'), 1)
        body = []
        for line in lines[lines.index('allowBuilds:') + 1:]:
            if line and not line.startswith(' '):
                break
            if line.strip():
                body.append(line)
        self.assertEqual(body, ['  esbuild: true'])
        for legacy in ('onlyBuiltDependencies:', 'onlyBuiltDependenciesFile:',
                       'ignoredBuiltDependencies:', 'neverBuiltDependencies:',
                       'dangerouslyAllowAllBuilds:'):
            self.assertNotIn(legacy, lines)


if __name__ == "__main__":
    unittest.main()
