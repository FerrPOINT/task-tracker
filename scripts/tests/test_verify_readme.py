import importlib.util
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "verify_readme.py"


def load_validator():
    spec = importlib.util.spec_from_file_location("verify_readme", SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class VerifyReadmeTests(unittest.TestCase):
    def make_repo(self, readme: str) -> Path:
        root = Path(tempfile.mkdtemp())
        (root / "README.md").write_text(readme, encoding="utf-8")
        return root

    def test_accepts_complete_readme_with_reviewed_evidence(self) -> None:
        validator = load_validator()
        root = self.make_repo(
            '<a name="overview"></a>\n'
            '<a name="capabilities"></a>\n'
            '<a name="quick-start"></a>\n'
            '<a name="visual-proof"></a>\n'
            '<a name="safety"></a>\n'
            '<a name="quality"></a>\n'
            '<a name="license"></a>\n'
            '### Projects (`wide`)\n![wide desktop](docs/screenshots/1920x1080/wide.png)\n'
            '### Create (`reading/form`)\n![reading desktop](docs/screenshots/1920x1080/reading.png)\n'
            '### Issue (`detail-with-aside`)\n![detail desktop](docs/screenshots/1920x1080/detail-with-aside.png)\n'
        )
        for name in validator.REQUIRED_PROOF:
            asset = root / "docs/screenshots" / name
            asset.parent.mkdir(parents=True, exist_ok=True)
            asset.write_bytes(b"png")

        self.assertEqual(validator.validate(root), [])

    def test_reports_missing_required_proof(self) -> None:
        validator = load_validator()
        root = self.make_repo('<a name="overview"></a>\n')

        findings = validator.validate(root)

        self.assertIn(
            "RMD004: README.md: missing proof asset: 1920x1080/wide.png",
            findings,
        )

    def test_rejects_mobile_screenshot_in_readme(self) -> None:
        validator = load_validator()
        root = self.make_repo("![Mobile](docs/screenshots/375x812/wide.png)\n")

        self.assertIn("RMD007", "\n".join(validator.validate(root)))



class ValidatorSourceRoutingTests(unittest.TestCase):
    """Exercise the packaged planner and its actual CI projection, without builders."""
    SOURCE = 'scripts/verify_readme.py'
    OWNER = 'tooling:quality-routes'
    MODULE = 'test_verify_readme'

    def plan_validator(self, paths, profile='build'):
        from unittest.mock import patch
        from scripts.vendor import base_quality_selection as selector
        root = Path(__file__).resolve().parents[2]
        def head(repository, *arguments):
            self.assertEqual(repository, root)
            self.assertEqual(arguments, ('rev-parse', 'HEAD'))
            return 'a' * 40
        with patch.object(selector, 'git', side_effect=head):
            return selector.plan(root, profile=profile, changed=paths)

    def test_validator_and_unit_paths_choose_exact_source_checks(self):
        for path in (self.SOURCE, 'scripts/tests/' + self.MODULE + '.py'):
            for profile in ('fast', 'unit', 'build'):
                with self.subTest(path=path, profile=profile):
                    candidate = self.plan_validator([path], profile)
                    expected = {self.OWNER}
                    if path == self.SOURCE and profile == 'fast':
                        expected.add('docs:readme')
                    self.assertEqual({gate['id'] for gate in candidate['gates']}, expected)
                    gate = next(g for g in candidate['gates'] if g['id'] == self.OWNER)
                    self.assertEqual(gate['argv'].count('scripts.tests.' + self.MODULE), 1)
                    self.assertFalse(any(g.get('kind') == 'artifact' for g in candidate['gates']))

    def test_mixed_application_and_unknown_neighbor_keep_builds(self):
        for paths in ([self.SOURCE, 'backend/src/main/java/example/Production.java'],
                      [self.SOURCE, self.SOURCE.replace('.py', '_unreviewed.py')]):
            with self.subTest(paths=paths):
                candidate = self.plan_validator(paths)
                self.assertIn('backend', candidate['components'])
                self.assertTrue(any(g.get('kind') == 'artifact' for g in candidate['gates']))

    def test_ci_projection_keeps_validator_only_out_of_application_jobs(self):
        from unittest.mock import patch
        from scripts.vendor import base_quality_selection as selector
        root = Path(__file__).resolve().parents[2]
        for paths, backend in (([self.SOURCE], 'false'),
                ([self.SOURCE, 'backend/src/main/java/example/Production.java'], 'true')):
            with self.subTest(paths=paths):
                def changed(repository, *arguments):
                    self.assertEqual(arguments, ('diff', '--name-only', '--no-renames', 'a' * 40, 'b' * 40))
                    return '\n'.join(paths)
                with patch.object(selector, 'git', side_effect=changed):
                    result = selector.ci_outputs('pull_request', 'a' * 40, 'b' * 40, repo=root)
                self.assertEqual(result['backend_build'], backend)
                self.assertEqual(result['frontend_build'], 'false')


if __name__ == "__main__":
    unittest.main()
