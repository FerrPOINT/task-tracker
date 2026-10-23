"""Known pure Python tests select no application producer."""
import shlex
import unittest
from pathlib import Path
from scripts.vendor import base_quality_selection as selection

ROOT = Path(__file__).resolve().parents[2]
TESTS = ('test_build_pin', 'test_verify_readme', 'test_ci_change_classifier', 'test_tooling_ownership')


class PythonToolingOwnershipTests(unittest.TestCase):
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
        for name in (*TESTS, 'test_ci_contract_checks'):
            with self.subTest(module=name):
                self.assertEqual(sum(c.count('scripts.tests.' + name) for c in commands), 1)
        self.assertIn('run: python3 scripts/verify_readme.py', ci)


if __name__ == '__main__':
    unittest.main()
