"""Known pure Python tests select no application producer."""
import shlex
import unittest
from pathlib import Path
from unittest.mock import patch
from scripts.vendor import base_quality_selection as selection

ROOT = Path(__file__).resolve().parents[2]
TESTS = ('test_build_pin', 'test_verify_readme', 'test_ci_change_classifier', 'test_tooling_ownership')


class PythonToolingOwnershipTests(unittest.TestCase):
    def test_manual_css_controls_keep_one_existing_vitest_run(self):
        import json
        scripts = json.loads((ROOT / 'frontend/package.json').read_text(encoding='utf-8'))['scripts']
        prefix = 'node --test scripts/tailwind-source-inputs.test.mjs && '
        self.assertEqual(scripts['test:all'], prefix + 'vitest run')
        self.assertTrue(scripts['test:unit'].startswith(prefix + 'vitest run --exclude '))
        self.assertEqual(scripts['build'].count('pnpm run test:unit'), 1)

    def test_css_scanner_test_selects_units_without_application_builds(self):
        manifest = selection.load(ROOT)
        path = 'frontend/scripts/tailwind-source-inputs.test.mjs'
        for profile in ('fast', 'unit', 'build'):
            with self.subTest(profile=profile):
                candidate = selection.plan(ROOT, profile=profile, changed=[path], manifest=manifest)
                self.assertIn('frontend:tests', [g['id'] for g in candidate['gates']])
                self.assertFalse(any(g.get('kind') == 'artifact' for g in candidate['gates']))
        mixed = selection.plan(ROOT, profile='build', changed=[path, 'frontend/src/index.css'], manifest=manifest)
        self.assertIn('frontend:build', [g['id'] for g in mixed['gates']])
        self.assertFalse(any(g['component'] == 'backend' for g in mixed['gates']))

    def test_ci_css_unit_source_preserves_real_application_build_selection(self):
        for paths, build in (('frontend/scripts/tailwind-source-inputs.test.mjs\n', 'false'),
                             ('frontend/scripts/tailwind-source-inputs.test.mjs\nfrontend/src/index.css\n', 'true')):
            with patch.object(selection, 'git', return_value=paths):
                values = selection.ci_outputs('push', 'a' * 40, 'b' * 40, repo=ROOT)
            self.assertEqual(values, {'backend': 'false', 'frontend': 'true',
                                      'backend_build': 'false', 'frontend_build': build})

    def test_posix_wrapper_test_selects_only_its_linux_unit(self):
        manifest = selection.load(ROOT)
        # Freeze only HEAD, not the selector's source/ownership logic. Planning
        # must never prepare a compiler or infer an application from this test.
        head = selection.git(ROOT, 'rev-parse', 'HEAD')
        with patch.object(selection, 'git', return_value=head):
            for profile in ('fast', 'unit', 'build'):
                for platform in ('win32', 'linux'):
                    with self.subTest(profile=profile, platform=platform):
                        candidate = selection.plan(ROOT, profile=profile,
                            changed=['scripts/tests/test_base_maintenance.py'],
                            manifest=manifest, execution_platform=platform)
                        self.assertEqual(candidate['components'], ['maintenance'])
                        self.assertEqual([g['id'] for g in candidate['gates']], ['maintenance:wrapper-unit'])
                        gate = candidate['gates'][0]
                        self.assertEqual(gate['platforms'], ['linux'])
                        self.assertEqual(gate['test_tier'], 'unit')
                        self.assertEqual(gate['execution_slot'], 'none')
                        self.assertFalse(gate.get('kind') == 'artifact')
                        self.assertFalse(gate.get('prepare_frontend'))
                        self.assertIn('scripts.tests.test_base_maintenance', gate['argv'])

    def test_posix_wrapper_unit_does_not_schedule_ci_app_jobs(self):
        with patch.object(selection, 'git', return_value='scripts/tests/test_base_maintenance.py\n'):
            for event in ('push', 'pull_request'):
                with self.subTest(event=event):
                    values = selection.ci_outputs(event, 'a' * 40, 'b' * 40, repo=ROOT)
                    self.assertEqual(values, {'backend': 'false', 'frontend': 'false',
                                              'backend_build': 'false', 'frontend_build': 'false'})

    def test_known_tests_select_only_python_helpers(self):
        manifest = selection.load(ROOT)
        for profile in ('fast', 'unit', 'build'):
            for name in TESTS:
                with self.subTest(profile=profile, name=name):
                    candidate = selection.plan(ROOT, profile=profile,
                        changed=['scripts/tests/' + name + '.py'], manifest=manifest)
                    self.assertEqual(candidate['components'], ['tooling'])
                    self.assertEqual([g['id'] for g in candidate['gates']], ['tooling:python-helpers'])
                    gate = candidate['gates'][0]
                    self.assertEqual(gate['argv'][0], '{python}')
                    self.assertFalse(gate.get('kind') == 'artifact')
                    self.assertTrue({'scripts.tests.' + n for n in TESTS} <= set(gate['argv']))

    def test_contract_test_keeps_its_existing_gate_and_unknown_script_stays_wide(self):
        contract = selection.plan(ROOT, profile='build', changed=['scripts/tests/test_ci_contract_checks.py'])
        self.assertEqual([g['id'] for g in contract['gates']], ['tooling:build-contracts'])
        for path, owner in [('scripts/new_generator.py', None), ('scripts/build.py', None),
                            ('frontend/src/product.tsx', 'frontend'), ('backend/src/lib.rs', 'backend')]:
            with self.subTest(path=path):
                candidate = selection.plan(ROOT, profile='build', changed=[path])
                if owner:
                    self.assertIn(owner, candidate['components'])
                else:
                    self.assertTrue({'backend', 'frontend'} <= set(candidate['components']))
                self.assertTrue(any(g.get('kind') == 'artifact' for g in candidate['gates']))

    def test_ci_includes_each_tooling_module_once(self):
        ci = (ROOT / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        commands = [shlex.split(line.strip().removeprefix('run: ')) for line in ci.splitlines()
                    if line.strip().removeprefix('run: ').startswith('python3 -m unittest ')]
        for name in (*TESTS, 'test_ci_contract_checks', 'test_base_maintenance'):
            with self.subTest(module=name):
                self.assertEqual(sum(c.count('scripts.tests.' + name) for c in commands), 1)
        self.assertIn('run: python3 scripts/verify_readme.py', ci)


if __name__ == '__main__':
    unittest.main()
