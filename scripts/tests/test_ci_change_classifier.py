"""Behavioral tests for the CI path classifier, including PR merge refs."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.classify_ci_changes import classify


class CiChangeClassifierTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="ci-path-classifier-")
        self.repo = Path(self.temporary.name)
        self.git("init", "-b", "main")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.invalid")
        (self.repo / "README.md").write_text("base\n", encoding="utf-8")
        self.git("add", "README.md")
        self.git("commit", "-m", "base")
        self.base = self.git("rev-parse", "HEAD")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args], cwd=self.repo, check=True, capture_output=True, text=True
        ).stdout.strip()

    def test_pr_uses_head_commit_and_ignores_base_changes_in_synthetic_merge(self) -> None:
        self.git("switch", "-c", "pr")
        (self.repo / "docs").mkdir()
        (self.repo / "docs" / "ops.md").write_text("docs\n", encoding="utf-8")
        self.git("add", "docs/ops.md")
        self.git("commit", "-m", "docs only")
        head = self.git("rev-parse", "HEAD")

        self.git("switch", "main")
        (self.repo / "backend").mkdir()
        (self.repo / "backend" / "Dockerfile.umbrella").write_text("base change\n", encoding="utf-8")
        self.git("add", "backend/Dockerfile.umbrella")
        self.git("commit", "-m", "independent base change")
        self.git("merge", "--no-ff", "pr", "-m", "synthetic pull request merge")
        merge = self.git("rev-parse", "HEAD")

        self.assertEqual(classify("pull_request", self.base, head, repo=self.repo),
                         {"backend": "false", "frontend": "false"})
        self.assertEqual(classify("pull_request", self.base, merge, repo=self.repo),
                         {"backend": "true", "frontend": "false"})

    def test_backend_and_shared_contract_paths_select_only_required_jobs(self) -> None:
        (self.repo / "backend").mkdir()
        (self.repo / "backend" / "lib.rs").write_text("code\n", encoding="utf-8")
        self.git("add", "backend/lib.rs")
        self.git("commit", "-m", "backend change")
        backend_head = self.git("rev-parse", "HEAD")
        self.assertEqual(classify("pull_request", self.base, backend_head, repo=self.repo),
                         {"backend": "true", "frontend": "false"})

        (self.repo / "openapi").mkdir()
        (self.repo / "openapi" / "openapi.json").write_text("{}\n", encoding="utf-8")
        self.git("add", "openapi/openapi.json")
        self.git("commit", "-m", "shared contract change")
        contract_head = self.git("rev-parse", "HEAD")
        self.assertEqual(classify("pull_request", self.base, contract_head, repo=self.repo),
                         {"backend": "true", "frontend": "true"})

    def test_non_pr_events_keep_full_checks_enabled(self) -> None:
        self.assertEqual(classify("push", "", "", repo=self.repo),
                         {"backend": "true", "frontend": "true"})

    def test_pull_request_requires_both_exact_shas(self) -> None:
        with self.assertRaises(ValueError):
            classify("pull_request", self.base, "", repo=self.repo)


if __name__ == "__main__":
    unittest.main()
