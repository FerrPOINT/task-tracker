#!/usr/bin/env python3
"""Regression checks for Task Tracker API contract gates in CI."""

from __future__ import annotations

import unittest
import re
import shlex
from pathlib import Path
from unittest.mock import patch

from scripts.vendor import base_quality_selection as selector

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/release.yml"
DOMAIN_TEST_MODULES = {
    "entities": ("entities/tests.rs", "tests", 11),
    "repositories": ("repositories/tests.rs", "tests", 10),
    "stubs/memory": ("stubs/memory/tests.rs", "tests", 10),
    "value_objects": ("value_objects/tests.rs", "tests", 3),
    "sdlc": ("tests/project_directory.rs", "project_directory_tests", 2),
    "jql": ("tests/jql.rs", "tests", 15),
    "sdlc_execution_lease": ("tests/sdlc_execution_lease.rs", "tests", 1),
    "sdlc_native_admission": ("tests/sdlc_native_admission.rs", "tests", 4),
    "sdlc_pm_draft": ("tests/sdlc_pm_draft.rs", "tests", 3),
}
RUST_TEST = r"#\[(?:tokio::)?test\]"


class CiContractChecksTest(unittest.TestCase):
    def test_frontend_uses_ui_pin_and_backend_retains_sdk_pin(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        frontend = re.split(r"\n  [a-zA-Z0-9_-]+:\n", workflow.split("\n  frontend:\n", 1)[1], 1)[0]
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


class DomainUnitInputContractTest(unittest.TestCase):
    def test_contract_script_has_its_own_static_component(self) -> None:
        candidate=selector.plan(ROOT,"unit",changed=["scripts/tests/test_ci_contract_checks.py"])
        self.assertEqual([gate["id"] for gate in candidate["gates"]],["tooling:build-contracts"])
        self.assertEqual(candidate["gates"][0]["argv"],["{python}","-B","-m","unittest","scripts.tests.test_ci_contract_checks"])

    def test_all_domain_units_are_cfg_test_modules_with_the_existing_inventory(self) -> None:
        domain = ROOT / "backend/domain/src"
        counted = 0
        for source, (relative, module, count) in DOMAIN_TEST_MODULES.items():
            with self.subTest(source=source):
                parent = domain / (source + ".rs")
                declaration = parent.read_text(encoding="utf-8")
                # entities.rs uses Rust's ordinary out-of-line module lookup.
                module_path = relative.removeprefix("stubs/")
                path = "" if source == "entities" else f'#[path = "{module_path}"]\n'
                self.assertIn(f"#[cfg(test)]\n{path}mod {module};", declaration)
                self.assertIsNone(re.search(RUST_TEST, declaration))
                body = (domain / relative).read_text(encoding="utf-8")
                self.assertEqual(len(re.findall(RUST_TEST, body)), count)
                counted += count
        self.assertEqual(counted, 59)
        self.assertEqual(
            sum(len(re.findall(RUST_TEST, path.read_text(encoding="utf-8")))
                for path in domain.rglob("*.rs")),
            counted,
        )

    def test_both_production_copies_exclude_only_domain_tests_and_keep_full_unit_copy(self) -> None:
        expected = {"task-tracker/backend/domain/src/" + row[0]
                    for row in DOMAIN_TEST_MODULES.values()}
        files = {"task-tracker/" + path.relative_to(ROOT).as_posix()
                 for path in (ROOT / "backend/domain").rglob("*") if path.is_file()}
        for recipe in ("Dockerfile", "Dockerfile.umbrella"):
            with self.subTest(recipe=recipe):
                text = (ROOT / "backend" / recipe).read_text(encoding="utf-8")
                unit = text.split(" AS unit\n", 1)[1].split("FROM dependencies AS builder", 1)[0]
                self.assertIn("COPY task-tracker/backend/domain ./domain\n", unit)
                self.assertNotIn("--exclude=", unit)
                production = text.split("FROM dependencies AS builder", 1)[1]
                copies = [shlex.split(line) for line in production.splitlines()
                          if line.startswith("COPY ") and "task-tracker/backend/domain" in line]
                self.assertEqual(len(copies), 1)
                self.assertEqual(copies[0][-2:], ["task-tracker/backend/domain", "./domain"])
                patterns = [arg.removeprefix("--exclude=") for arg in copies[0][1:-2]]
                self.assertEqual(len(patterns), len(expected))
                self.assertTrue(all(arg.startswith("--exclude=**/src/") for arg in copies[0][1:-2]))
                self.assertEqual({name for name in files if selector.matches(name, patterns)}, expected)

    def test_test_only_inputs_skip_artifact_key_plan_and_ci_build(self) -> None:
        manifest = selector.load(ROOT)
        build = next(gate for gate in manifest["gates"] if gate["id"] == "backend:build")
        for relative, _, _ in DOMAIN_TEST_MODULES.values():
            name = "backend/domain/src/" + relative
            with self.subTest(path=name):
                self.assertTrue(selector.matches(name, build["inputs"]))
                self.assertTrue(selector.matches(name, build["exclude_inputs"]))
                self.assertTrue(selector.matches(name, manifest["backend_unit_inputs"]))
                with patch.object(selector, "git", return_value=name + "\n"):
                    gates = selector.plan(ROOT, "build", changed=[name])["gates"]
                    ci = selector.ci_outputs("push", "a" * 40, "b" * 40, repo=ROOT)
                self.assertEqual([gate["id"] for gate in gates], ["backend:unit"])
                self.assertEqual(ci["backend"], "true")
                self.assertEqual(ci["backend_build"], "false")

    def test_production_neighbors_still_select_and_key_the_artifact(self) -> None:
        manifest = selector.load(ROOT)
        build = next(gate for gate in manifest["gates"] if gate["id"] == "backend:build")
        for source in DOMAIN_TEST_MODULES:
            name = "backend/domain/src/" + source + ".rs"
            with self.subTest(path=name):
                self.assertTrue(selector.matches(name, build["inputs"]))
                self.assertFalse(selector.matches(name, build["exclude_inputs"]))
                self.assertFalse(selector.matches(name, manifest["backend_unit_inputs"]))
                with patch.object(selector, "git", return_value=name + "\n"):
                    gates = selector.plan(ROOT, "build", changed=[name])["gates"]
                    ci = selector.ci_outputs("push", "a" * 40, "b" * 40, repo=ROOT)
                self.assertEqual([gate["id"] for gate in gates], ["backend:unit", "backend:build"])
                self.assertEqual(ci["backend_build"], "true")


if __name__ == "__main__":
    unittest.main()
