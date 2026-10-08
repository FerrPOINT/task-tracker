"""POSIX wrappers forward the scoped Base contract, without touching Docker."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(os.name == "posix" and shutil.which("bash"), "requires POSIX bash")
class BaseMaintenanceTests(unittest.TestCase):
    def invoke(self, operation, *, project="sdlc2", missing=None, arguments=None):
        with tempfile.TemporaryDirectory(prefix="tracker maintenance ") as temporary:
            path = Path(temporary)
            runner = path / "python3"
            runner.write_text(
                f"#!{sys.executable}\nimport json, os, sys\n"
                "with open(os.environ['RECEIPT'], 'w') as stream:\n"
                "    json.dump(sys.argv[1:], stream)\n",
                encoding="utf-8",
            )
            runner.chmod(0o700)
            docker = path / "docker"
            docker.write_text("#!/bin/sh\nexit 99\n", encoding="utf-8")
            docker.chmod(0o700)
            env = {key: value for key, value in os.environ.items() if not key.startswith("SDLC_")}
            env.update(
                PATH=str(path) + os.pathsep + os.environ["PATH"],
                SDLC_TASK="tracker-maintenance-test",
                SDLC_WORKSPACE_DIR=str(path),
                SDLC_PROJECT=project,
                SDLC_DOCKER_CONTEXT="selected-endpoint",
                SDLC_SIGNING_KEY=str(path / "preserved key.pem"),
                RECEIPT=str(path / "receipt.json"),
            )
            if missing:
                env.pop(missing)
            archive = str(path / "protected archive.tar.gz")
            result = subprocess.run(
                ["bash", str(ROOT / "scripts" / f"{operation}.sh"),
                 *(arguments if arguments is not None else [archive])],
                env=env, capture_output=True, text=True,
            )
            receipt = path / "receipt.json"
            args = json.loads(receipt.read_text()) if receipt.exists() else []
            return result.returncode, args, path, archive

    def test_both_wrappers_forward_exact_profile_and_owner(self):
        for operation in ("backup", "restore"):
            for project in ("sdlc1", "sdlc2"):
                with self.subTest(operation=operation, project=project):
                    code, args, path, archive = self.invoke(operation, project=project)
                    self.assertEqual(code, 0)
                    expected = [
                        str(path / "services-base/scripts/platform_backup.py"), operation,
                        "--project", project, "--workspace-profile", str(path / "workspace.local.json"),
                        "--compose-file", str(path / "docker-compose.local.yml"),
                        "--project-directory", str(path), "--docker-context", "selected-endpoint",
                        "--task", "tracker-maintenance-test", "--layout", "auto",
                    ]
                    expected += (["--quiesce", "--signing-key"] if operation == "backup"
                                 else ["--qa-only", "--signing-key-target"])
                    expected += [str(path / "preserved key.pem"),
                                 "--output" if operation == "backup" else "--archive", archive]
                    self.assertEqual(args, expected)

    def test_missing_required_environment_never_calls_base(self):
        for operation in ("backup", "restore"):
            for name in ("SDLC_TASK", "SDLC_WORKSPACE_DIR", "SDLC_PROJECT",
                         "SDLC_DOCKER_CONTEXT", "SDLC_SIGNING_KEY"):
                with self.subTest(operation=operation, name=name):
                    code, args, _, _ = self.invoke(operation, missing=name)
                    self.assertNotEqual(code, 0)
                    self.assertEqual(args, [])

    def test_unknown_and_physical_projects_never_call_base(self):
        for operation in ("backup", "restore"):
            for project in ("sdlc-demo", "sdlc-common", "sdlc-qa-restore-example"):
                with self.subTest(operation=operation, project=project):
                    code, args, _, _ = self.invoke(operation, project=project)
                    self.assertNotEqual(code, 0)
                    self.assertEqual(args, [])

    def test_missing_or_extra_arguments_never_call_base(self):
        for operation in ("backup", "restore"):
            for arguments in ([], ["archive", "--allow-source-project"]):
                with self.subTest(operation=operation, arguments=arguments):
                    code, args, _, _ = self.invoke(operation, arguments=arguments)
                    self.assertNotEqual(code, 0)
                    self.assertEqual(args, [])


if __name__ == "__main__":
    unittest.main()
