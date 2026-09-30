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


if __name__ == "__main__":
    unittest.main()
