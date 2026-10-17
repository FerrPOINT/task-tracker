"""Source contracts for the umbrella Cargo budget and packaged server."""

from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]


class DockerfileBudgetTests(unittest.TestCase):
    def setUp(self):
        self.recipe = (ROOT / "backend/Dockerfile.umbrella").read_text(
            encoding="utf-8"
        )

    def test_single_cargo_worker_default_precedes_build_in_builder_stage(self):
        builder, runtime = self.recipe.split("FROM debian:bookworm-slim", 1)
        self.assertEqual(builder.count("CARGO_BUILD_JOBS"), 1)
        self.assertIn("ARG CARGO_BUILD_JOBS=1", builder.splitlines())
        self.assertLess(
            builder.index("ARG CARGO_BUILD_JOBS=1"),
            builder.index("RUN --mount=type=cache"),
        )
        self.assertNotIn("CARGO_BUILD_JOBS", runtime)

    def test_locked_release_server_build_keeps_shared_cache_paths(self):
        self.assertIn(
            "--mount=type=cache,id=task-tracker-cargo-registry-rust-1.88,"
            "target=/usr/local/cargo/registry,sharing=shared",
            self.recipe,
        )
        self.assertIn(
            "--mount=type=cache,id=task-tracker-cargo-target-rust-1.88,"
            "target=/cargo-target,sharing=locked",
            self.recipe,
        )
        self.assertIn(
            "CARGO_TARGET_DIR=/cargo-target cargo build --locked --release --bin server",
            self.recipe,
        )
        self.assertNotIn("--jobs", self.recipe)
        self.assertNotIn(" -j", self.recipe)

    def test_declared_server_target_matches_install_copy_and_runtime_command(self):
        manifest = tomllib.loads(
            (ROOT / "backend/server/Cargo.toml").read_text(encoding="utf-8")
        )
        target = next(entry for entry in manifest["bin"] if entry["name"] == "server")
        self.assertTrue((ROOT / "backend/server" / target["path"]).is_file())
        self.assertIn(
            "install -m 0755 /cargo-target/release/server /app/task-tracker-server",
            self.recipe,
        )
        self.assertIn(
            "COPY --from=builder /app/task-tracker-server "
            "/usr/local/bin/task-tracker-server",
            self.recipe.splitlines(),
        )
        self.assertIn('CMD ["task-tracker-server"]', self.recipe.splitlines())


if __name__ == "__main__":
    unittest.main()
