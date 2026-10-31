"""Selection and command contracts; no compiler, install, Docker or runtime."""

import json
import re
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.vendor import base_quality_selection as selector

ROOT = Path(__file__).resolve().parents[2]


class CurrentPerformanceRoutesTests(unittest.TestCase):
    def plan(self, components, profile):
        with patch.object(selector, 'git', return_value='a' * 40):
            return selector.plan(ROOT, profile=profile, changed=[], components=components)

    def test_node_matches_the_current_product_in_every_preparation_route(self):
        gates = [gate for gate in selector.load(ROOT)['gates'] if gate.get('prepare_frontend')]
        self.assertEqual(len(gates), 5)
        self.assertEqual({gate['prepare_frontend'] for gate in gates}, {'22.22.2'})
        for gate in gates:
            tool = next(tool for tool in gate['tools'] if tool['argv'] == ['node', '--version'])
            self.assertRegex('v22.22.2\n', tool['pattern'])
            self.assertNotRegex('v22.20.0\n', tool['pattern'])
        for name in ('frontend/Dockerfile', 'frontend/Dockerfile.umbrella'):
            self.assertIn('FROM node:22.22.2-bookworm-slim', (ROOT / name).read_text(encoding='utf-8'))
        for name in ('ci.yml', 'qualification.yml'):
            self.assertIn('node-version: 22.22.2', (ROOT / '.github/workflows' / name).read_text(encoding='utf-8'))

    def test_each_rust_component_retains_full_manual_coverage_without_duplicate_units(self):
        for component in ('backend', 'cli'):
            with self.subTest(component=component):
                gates = self.plan([component], 'release')['gates']
                self.assertEqual(len(gates), 1)
                self.assertIn('cargo test --locked --workspace', gates[0]['argv'][-1])
                self.assertFalse(gates[0]['cacheable'])
                self.assertEqual(gates[0]['requires_env'], ['TT_TEST_DATABASE_URL'])
                self.assertEqual(gates[0]['sdk_revision'], '.namespace-base-revision')
        combined = self.plan(['backend', 'cli'], 'release')['gates']
        self.assertEqual(len(combined), 1)
        self.assertEqual({row['component'] for row in combined[0]['shared_check_members']}, {'backend', 'cli'})

    def test_full_frontend_replaces_the_unit_subset_only_at_the_manual_milestone(self):
        automatic = self.plan(['frontend'], 'build')['gates']
        self.assertIn('frontend:tests', {gate['id'] for gate in automatic})
        self.assertNotIn('frontend:full-tests', {gate['id'] for gate in automatic})
        manual = self.plan(['frontend'], 'release')['gates']
        self.assertNotIn('frontend:tests', {gate['id'] for gate in manual})
        full = next(gate for gate in manual if gate['id'] == 'frontend:full-tests')
        self.assertEqual(full['argv'], ['pnpm', 'test:all'])
        self.assertFalse(full['cacheable'])
        source = (ROOT / '.github/workflows/qualification.yml').read_text(encoding='utf-8')
        self.assertEqual(source.count('run: pnpm test:all'), 1)
        self.assertNotIn('run: pnpm test:unit', source)
        self.assertEqual(source.count('cargo test --locked --workspace -- --test-threads=1'), 1)
        self.assertNotIn('cargo test --locked -p domain --lib', source)

    def test_formatter_uses_local_files_and_compilers_keep_sdk_verification(self):
        gates = {gate['id']: gate for gate in selector.load(ROOT)['gates']}
        gate = gates['cli:format']
        self.assertEqual(gate['argv'], ['rustfmt', '--edition', '2024', '--check', 'cli/src/main.rs'])
        self.assertNotIn('sdk_revision', gate)
        self.assertEqual(gate['test_tier'], 'static')
        for name in ('backend:unit', 'backend:build', 'cli:build'):
            self.assertEqual(gates[name]['sdk_revision'], '.namespace-base-revision')

    def test_automatic_ci_contains_no_manual_database_or_full_suite_work(self):
        source = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        self.assertNotIn('services:', source)
        self.assertNotIn('createdb ', source)
        self.assertNotIn('cargo test --locked --workspace', source)
        self.assertNotIn('cargo clippy --locked --workspace', source)
        self.assertIn('scripts.tests.test_current_performance_routes', source)
        self.assertIn('cargo test --locked -p domain --lib', source)
        for name in ('ci.yml', 'qualification.yml'):
            value = (ROOT / '.github/workflows' / name).read_text(encoding='utf-8')
            checkouts = re.findall(r'^          path: task-tracker\n(.*?)(?=^      -|^  [a-zA-Z0-9_-]+:|\Z)',
                                   value, re.MULTILINE | re.DOTALL)
            self.assertTrue(checkouts)
            for checkout in checkouts:
                self.assertIn('ref: ${{ github.event.pull_request.head.sha || github.sha }}', checkout)

    def test_just_preserves_manual_commands_and_requires_a_build_scope(self):
        source = (ROOT / 'justfile').read_text(encoding='utf-8')
        self.assertRegex(source, r'(?m)^build:\n    @echo .*; exit 64$')
        self.assertIn('build-all: build-backend build-cli build-frontend', source)
        self.assertIn('@test: test-backend test-frontend', source)
        self.assertIn('cargo test --locked -p domain --lib', source)
        self.assertIn('cargo build --locked --release -p server --bin server', source)
        self.assertIn('cargo build --locked --release -p task-tracker-cli --bin task-tracker', source)
        self.assertIn('gate: repository-hygiene fmt-check-rust clippy typecheck lint test-frontend-all test-backend-all', source)
        self.assertIn('api-codegen:\n    cd frontend && pnpm run postinstall', source)
        package = json.loads((ROOT / 'frontend/package.json').read_bytes())
        self.assertEqual(package['scripts']['test'], 'pnpm run test:unit')
        self.assertEqual(package['scripts']['build:compile'], 'vite build')


if __name__ == '__main__':
    unittest.main()
