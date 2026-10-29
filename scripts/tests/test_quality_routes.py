"""Pure Tracker selection and caller contracts; real Git/CLI is qualified separately."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import runpy
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

from scripts.vendor import base_quality_selection as selector
from scripts.select_ci import auto_services, build_services, outputs

ROOT = Path(__file__).resolve().parents[2]


class QualityRouteTests(unittest.TestCase):
    def setUp(self):
        def fixture_git(_root, *arguments):
            if arguments == ('rev-parse', 'HEAD'):
                return 'b' * 40
            raise AssertionError('Unexpected transport in path unit: ' + repr(arguments))
        self.git = patch.object(selector, 'git', side_effect=fixture_git)
        self.git.start()
        self.addCleanup(self.git.stop)

    def plan(self, path=None):
        return selector.plan(ROOT, profile='build', changed=[] if path is None else [path])

    def artifacts(self, path):
        return [g['id'] for g in self.plan(path)['gates'] if g.get('kind') == 'artifact']

    def test_no_change_docs_and_own_source_tools_do_not_select_application(self):
        self.assertEqual(self.plan()['gates'], [])
        self.assertEqual(self.artifacts('docs/TESTING.md'), [])
        self.assertEqual(self.artifacts('scripts/tests/test_quality_routes.py'), [])

    def test_frontend_and_ui_pin_are_independent_of_rust(self):
        for path in ('frontend/src/main.tsx', '.base-ui-revision'):
            self.assertEqual(self.plan(path)['components'], ['frontend'])
            self.assertEqual(self.artifacts(path), ['frontend:build'])

    def test_frontend_tests_setup_and_test_config_do_not_compile_vite(self):
        for path in ('frontend/src/api/client.test.ts', 'frontend/src/test/setup.ts',
                     'frontend/vitest.config.ts', 'frontend/scripts/tailwind-source-inputs.test.mjs'):
            self.assertEqual(self.artifacts(path), [])
            self.assertIn('frontend:tests', [g['id'] for g in self.plan(path)['gates']])

    def test_server_and_official_cli_are_separate_producers(self):
        self.assertEqual(self.artifacts('backend/server/src/main.rs'), ['backend:build'])
        self.assertEqual(self.artifacts('backend/cli/src/main.rs'), ['cli:build'])
        for path in ('backend/Cargo.lock', '.namespace-base-revision'):
            self.assertEqual(self.artifacts(path), ['backend:build', 'cli:build'])

    def test_external_rust_tests_keep_production_artifacts(self):
        for path in ('backend/infra/tests/namespace.rs', 'backend/cli/tests/http.rs'):
            self.assertEqual(self.artifacts(path), [])

    def test_unknown_source_fails_wide_instead_of_omitting_new_component(self):
        for path in ('backend/new-crate/src/main.rs', 'scripts/new-generator.py'):
            self.assertEqual(len(self.artifacts(path)), 3)
            self.assertEqual(set(self.plan(path)['components']), set(selector.load(ROOT)['components']))

    def test_shipping_targets_do_not_build_the_entire_workspace(self):
        gates = {g['id']: g for g in selector.load(ROOT)['gates']}
        self.assertIn('-p server --bin server', gates['backend:build']['argv'][-1])
        self.assertIn('-p task-tracker-cli --bin task-tracker', gates['cli:build']['argv'][-1])
        self.assertNotIn('--workspace', gates['backend:build']['argv'][-1])
        self.assertNotIn('--workspace', gates['cli:build']['argv'][-1])

    def test_units_and_static_checks_precede_any_producer(self):
        gates = self.plan('scripts/new-generator.py')['gates']
        first = next(i for i, g in enumerate(gates) if g.get('kind') == 'artifact')
        self.assertTrue(all(i < first for i, g in enumerate(gates) if g.get('test_tier') in ('unit','static')))



    def test_compiled_target_is_fresh_and_download_caches_survive(self):
        source = (ROOT / 'backend/Dockerfile.umbrella').read_text(encoding='utf-8')
        self.assertNotIn('target=/cargo-target,sharing=', source)
        self.assertIn('target=/usr/local/cargo/registry', source)
        self.assertIn('target=/usr/local/cargo/git', source)
        self.assertIn('cargo test --locked --release -p domain --lib', source)

    def test_ci_does_not_restore_unqualified_compiled_targets(self):
        for filename in ('ci.yml', 'qualification.yml'):
            text = (ROOT / '.github/workflows' / filename).read_text(encoding='utf-8')
            parts = text.split('uses: Swatinem/rust-cache@v2')[1:]
            self.assertTrue(parts)
            for part in parts:
                cache = part.split('\n      - ', 1)[0]
                self.assertIn('cache-targets: false', cache)
                self.assertIn('workspaces: task-tracker/backend', cache)

    def test_packaged_selector_matches_its_independent_revision(self):
        receipt = json.loads((ROOT / 'scripts/vendor/base-quality-tooling.json').read_bytes())
        self.assertEqual(receipt['revision'], (ROOT / '.quality-tooling-revision').read_text().strip())
        self.assertEqual(receipt['sha256'], hashlib.sha256((ROOT / 'scripts/vendor/base_quality_selection.py').read_bytes()).hexdigest())

    def ci(self, path):
        def fixture_git(_root, *arguments):
            if arguments == ('rev-parse', 'HEAD'): return 'b' * 40
            if arguments == ('diff', '--name-only', '--no-renames', 'a' * 40, 'b' * 40): return path + '\n'
            raise AssertionError(arguments)
        with patch.object(selector, 'git', side_effect=fixture_git):
            return outputs(ROOT, 'pull_request', 'a' * 40, 'b' * 40)

    def test_ci_and_container_caller_consume_the_same_plan(self):
        self.assertTrue(self.ci('backend/server/src/main.rs')['backend_build'])
        cli = self.ci('backend/cli/src/main.rs')
        self.assertTrue(cli['cli_build'])
        self.assertFalse(cli['server_checks'])
        self.assertFalse(cli['backend_build'])
        self.assertEqual(build_services(self.plan('backend/cli/src/main.rs')), [])
        self.assertEqual(build_services(self.plan('frontend/src/main.tsx')), ['frontend'])
        self.assertEqual(build_services(self.plan('backend/server/src/main.rs')), ['backend'])
        self.assertFalse(self.ci('frontend/src/api/client.test.ts')['frontend_build'])
        self.assertTrue(self.ci('openapi/openapi.json')['openapi_check'])

    def test_recipe_only_changes_select_packaging_without_application_compile(self):
        for path, service in [('backend/Dockerfile.umbrella', 'backend'),
                              ('backend/Dockerfile.umbrella.dockerignore', 'backend'),
                              ('frontend/Dockerfile', 'frontend'),
                              ('frontend/nginx.conf', 'frontend')]:
            plan = self.plan(path)
            self.assertEqual([g['id'] for g in plan['gates'] if g.get('kind') == 'artifact'], [])
            self.assertEqual(build_services(plan), [service])

    def test_push_uses_the_same_exact_changed_input_plan_as_pull_request(self):
        def fixture_git(_root, *arguments):
            if arguments == ('rev-parse', 'HEAD'): return 'b' * 40
            if arguments == ('diff', '--name-only', '--no-renames', 'a' * 40, 'b' * 40):
                return 'frontend/src/main.tsx\n'
            raise AssertionError(arguments)
        with patch.object(selector, 'git', side_effect=fixture_git):
            actual = outputs(ROOT, 'push', 'a' * 40, 'b' * 40)
        self.assertTrue(actual['frontend_build'])
        self.assertFalse(actual['backend_build'])
        self.assertFalse(actual['cli_build'])

    def test_invalid_ci_base_fails_before_git_or_preparation(self):
        with patch.object(selector, 'git') as git:
            with self.assertRaises(ValueError): outputs(ROOT, 'pull_request', '', 'b' * 40)
            git.assert_not_called()
        self.assertTrue(outputs(ROOT, 'push', '0' * 40, 'b' * 40)['frontend_build'])


    def test_automatic_generator_adapter_preserves_the_public_manual_route(self):
        package = json.loads((ROOT / 'frontend/package.json').read_bytes())
        receipt = json.loads((ROOT / 'scripts/vendor/base-quality-tooling.json').read_bytes())
        self.assertEqual(receipt['schema'], 'base-quality-tooling/v2')
        generator = receipt['frontend_generator']
        self.assertEqual(set(generator), {'source', 'file', 'contract', 'sha256'})
        self.assertEqual(generator['source'], 'scripts/generate_openapi.mjs')
        self.assertEqual(generator['file'], 'frontend/scripts/base-generate-openapi.mjs')
        self.assertEqual(generator['contract'], 'base-openapi-generation/v1')
        self.assertEqual(hashlib.sha256((ROOT / generator['file']).read_bytes()).hexdigest(), generator['sha256'])
        self.assertEqual(package['scripts']['postinstall'], 'node scripts/base-generate-openapi.mjs --schema ../openapi/openapi.json --output src/api/generated.ts')
        self.assertEqual(package['scripts']['generate:api'], 'openapi-typescript ../openapi/openapi.json -o src/api/generated.ts')
        for name in ('Dockerfile', 'Dockerfile.umbrella'):
            path = ROOT / 'frontend' / name
            if not path.is_file(): continue
            source = path.read_text()
            generation = next(i for i, line in enumerate(source.splitlines()) if line.startswith('RUN ')
                              and ('pnpm run postinstall' in line or 'pnpm install' in line and '--ignore-scripts' not in line))
            self.assertIn('base-generate-openapi.mjs', '\n'.join(source.splitlines()[:generation]))



class RawContainerScopeTests(unittest.TestCase):
    def invoke(self, targets, selected, pins):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); script = root / 'scripts/build.py'; script.parent.mkdir()
            shutil.copyfile(ROOT / 'scripts/build.py', script)
            for name, value in pins.items(): (root / name).write_text(value)
            with patch.object(sys, 'argv', [str(script), *targets]), patch('scripts.select_ci.auto_services', return_value=selected), patch('subprocess.run') as run, contextlib.redirect_stdout(io.StringIO()):
                runpy.run_path(str(script), run_name='__main__')
                return run.call_args_list

    def test_empty_changed_plan_skips_verifier_and_docker(self):
        self.assertEqual(self.invoke([], [], {}), [])

    def test_automatic_frontend_keeps_independent_ui_verifier(self):
        calls = self.invoke([], ['frontend'], {'.base-ui-revision':'1'*40,'.namespace-base-revision':'2'*40})
        self.assertEqual(len(calls), 2)
        self.assertTrue(calls[0].args[0][-1].endswith('.base-ui-revision'))
        self.assertEqual(calls[1].args[0], ['docker','compose','build','frontend'])

    def test_mixed_pin_context_refuses_before_any_docker_work(self):
        with self.assertRaisesRegex(SystemExit, 'Mixed SDK/UI pins'):
            self.invoke(['--all'], [], {'.base-ui-revision':'1'*40,'.namespace-base-revision':'2'*40})
