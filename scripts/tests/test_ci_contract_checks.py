#!/usr/bin/env python3
"""Regression checks for Task Tracker API contract gates in CI."""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/release.yml"


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

if __name__ == "__main__":
    unittest.main()
