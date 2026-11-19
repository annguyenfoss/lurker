"""Host-independent runner tests: no real mount, crypto operation or SSH."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load("remote_runner", "scripts/remote-test/runner.py")
coordinator = load("remote_coordinator", "scripts/remote-test.py")


class RemoteRunnerTests(unittest.TestCase):
    def test_exact_tree_verification_detects_mutation_and_extra_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "test").write_text("original")
            manifest = {"files": {"test": {"sha256": runner.digest(root / "test")}}}
            runner.verify_source(root, manifest)
            (root / "extra").write_text("unexpected")
            with self.assertRaisesRegex(ValueError, "unexpected"):
                runner.verify_source(root, manifest)
            (root / "extra").unlink()
            (root / "test").write_text("changed")
            with self.assertRaisesRegex(ValueError, "differs"):
                runner.verify_source(root, manifest)

    def test_refreshes_archive_mtime_without_changing_source_content_or_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary)
            archived = source / "lib.rs"
            archived.write_text("pub fn from_this_revision() {}")
            alias = source / "alias.rs"
            alias.symlink_to("lib.rs")
            os.utime(archived, (1, 1))
            os.utime(alias, (1, 1), follow_symlinks=False)
            manifest = {"files": {"lib.rs": {"sha256": runner.digest(archived)},
                                  "alias.rs": {"symlink": "lib.rs"}}}

            runner.refresh_source_mtimes(source, manifest)

            self.assertGreater(archived.stat().st_mtime, 1)
            self.assertEqual(1, alias.lstat().st_mtime)
            runner.verify_source(source, manifest)

    def test_timeout_kills_only_launched_process_group(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage = {"name": "timeout-test", "timeout": 0.1,
                     "commands": [[sys.executable, "-c", "import time;time.sleep(60)"]]}
            result = runner.execute(stage, root, root, os.environ.copy())
            self.assertEqual("timeout", result["status"])
            self.assertNotEqual(0, result["exit_code"])
            self.assertLess(result["duration_seconds"], 8)

    def test_counts_and_bounded_failure_excerpt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            code = "print('x'*20000);print('test result: FAILED. 2 passed; 1 failed; 3 ignored;');raise SystemExit(7)"
            result = runner.execute({"name": "rust-tests", "commands": [[sys.executable, "-c", code]]}, root, root, os.environ.copy())
            self.assertEqual("failed", result["status"])
            self.assertEqual(7, result["exit_code"])
            self.assertEqual({"passed": 2, "failed": 1, "skipped": 3}, result["test_counts"])
            self.assertLessEqual(len(result["diagnostic"].encode()), 8192)
            self.assertIn("1 failed", result["diagnostic"])

    def test_required_block_stops_dependent_commands_and_prevents_restart(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            job = root / "runs" / "job"
            source = job / "source"
            source.mkdir(parents=True)
            spec = {"job_id": "job", "run_id": "run", "commit": "a" * 40,
                    "host": {"os": runner.platform.system(), "arch": runner.platform.machine()}, "suite": "native",
                    "minimum_free_bytes": 1, "stages": [
                        {"name": "backend-build", "blocked": "required backend not built"},
                        {"name": "native-tests", "commands": [["must-not-run"]]}]}
            (job / "spec.json").write_text(json.dumps(spec))
            (job / "source-manifest.json").write_text(json.dumps({"files": {}}))
            (root / "cache").mkdir()
            with mock.patch.object(runner, "selected_environment", return_value=(os.environ.copy(), {}, root / "cache")):
                self.assertEqual(1, runner.run_job(job))
            result = json.loads((job / "result.json").read_text())
            self.assertEqual("blocked", result["status"])
            self.assertEqual("backend-build", result["failing_stage"])
            self.assertFalse(any(s["name"] == "native-tests" for s in result["stages"]))
            self.assertEqual(2, runner.run_job(job))
            self.assertFalse((root / "active-job").exists())

    def test_cancel_is_cooperative_and_does_not_use_stored_pid(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "cancel").touch()
            result = runner.execute({"name": "cancel-test", "timeout": 10,
                                     "commands": [[sys.executable, "-c", "import time;time.sleep(60)"]]},
                                    root, root, os.environ.copy())
            self.assertEqual("cancelled", result["status"])

    def test_invalid_stage_names_and_shell_commands_are_rejected(self):
        for stage in [{"name": "../escape", "commands": [["true"]]},
                      {"name": "test", "commands": ["sh -c bad"]},
                      {"name": "test", "skip": "no", "blocked": "yes"}]:
            with self.assertRaises(ValueError):
                coordinator.validate_spec({"stages": [stage]})

    def test_missing_runner_is_incomplete_not_running_or_passed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "launcher.json").write_text(json.dumps({"pid": 99999999, "signature": "old process"}))
            (root / "result.json").write_text(json.dumps({"status": "running"}))
            result = subprocess.run([sys.executable, "-c", coordinator.INSPECT, str(root)],
                                    capture_output=True, text=True, check=True)
            state = json.loads(result.stdout)
            self.assertEqual("incomplete", state["status"])
            self.assertTrue(state["runner_terminal"])

    def test_artifact_cannot_escape_target(self):
        for artifact in ("/etc/passwd", "../../private-key"):
            with self.assertRaisesRegex(ValueError, "artifact"):
                coordinator.validate_spec({"stages": [], "artifacts": [artifact]})

    def test_cancelled_queued_job_is_never_deployed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            job = {"job_id": "run-host", "host": {"ssh": "unused"}, "status": "queued"}
            manifest = {"run_id": "run", "commit": "a" * 40, "concurrency": 1, "jobs": [job], "options": {}}
            (root / "source.tar").write_bytes(b"")
            (root / "source-manifest.json").write_text("{}")
            (root / "run-host.cancel").touch()
            with mock.patch.object(coordinator, "read_run", return_value=(root, manifest)), mock.patch.object(coordinator, "deploy") as deploy:
                self.assertEqual(0, coordinator.dispatch(type("Args", (), {"run_id": "run"})()))
                deploy.assert_not_called()
            self.assertEqual("cancelled", job["status"])

    def test_compiler_cache_defaults_to_home_not_temporary_job_disk(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            home = root / "home"
            home.mkdir()
            job = root / "temporary-jobs/runs/job"
            job.mkdir(parents=True)
            (job / "source-manifest.json").write_text(json.dumps({"files": {}}))
            spec = {"host": {"rust": "rustup"}, "stages": [], "commit": "a" * 40, "minimum_cache_free_bytes": 1}
            output = type("Process", (), {"returncode": 0, "stdout": "version", "stderr": ""})()
            with mock.patch.dict(os.environ, {"HOME": str(home)}), mock.patch.object(runner.subprocess, "run", return_value=output):
                env, _, cache = runner.selected_environment(spec, job)
            self.assertTrue(cache.is_relative_to(home / ".cache/lurker/remote-test"))
            self.assertFalse(cache.is_relative_to(job.parents[1]))
            self.assertEqual(str(cache / "target"), env["CARGO_TARGET_DIR"])

    def test_explicit_rust_bin_uses_existing_shims_and_records_selected_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary_root = root / "managed-shims"
            binary_root.mkdir()
            for name in ("cargo", "rustc"):
                path = binary_root / name
                path.write_text("fixture; never executed")
                path.chmod(0o755)
            job = root / "jobs/run"
            job.mkdir(parents=True)
            (job / "source-manifest.json").write_text(json.dumps({"files": {}}))
            spec = {"host": {"rust": "rustup", "rust_bin": str(binary_root)}, "stages": [],
                    "commit": "a" * 40, "cache_root": str(root / "cache"), "minimum_cache_free_bytes": 1}
            output = type("Process", (), {"returncode": 0, "stdout": "version", "stderr": ""})()
            with mock.patch.object(runner.subprocess, "run", return_value=output):
                env, versions, _ = runner.selected_environment(spec, job)
            self.assertEqual(str(binary_root), env["PATH"].split(os.pathsep)[0])
            self.assertEqual(str(binary_root / "cargo"), versions["cargo"]["path"])
            spec["host"]["rust_bin"] = "relative/shims"
            with self.assertRaisesRegex(ValueError, "absolute toolchain"):
                runner.selected_environment(spec, job)

    def test_uv_python_resolution_is_offline_and_never_falls_back(self):
        host = {"ssh": "adjutant@buran", "uv": "/opt/homebrew/bin/uv", "python_request": "3.15"}
        path = "/Users/adjutant/.local/share/uv/python/current/bin/python3.15"
        output = type("Process", (), {"returncode": 0, "stdout": path + "\n", "stderr": ""})()
        with mock.patch.object(coordinator.subprocess, "run", return_value=output) as run:
            resolved = coordinator.resolve_python(host)
        self.assertEqual(path, resolved["python"])
        command = run.call_args_list[0].args[0][-1]
        for flag in ("--offline", "--managed-python", "--no-python-downloads", "--no-project", "--no-config"):
            self.assertIn(flag, command)
        failed = type("Process", (), {"returncode": 1, "stdout": "", "stderr": "managed Python missing"})()
        with mock.patch.object(coordinator.subprocess, "run", return_value=failed) as run:
            with self.assertRaisesRegex(RuntimeError, "configured Python/uv unavailable"):
                coordinator.resolve_python(host)
            self.assertEqual(1, run.call_count)
        with self.assertRaisesRegex(ValueError, "absolute path"):
            coordinator.resolve_python(dict(host, uv="../uv"))

    def test_native_recipe_changes_preserve_cargo_cache_identity(self):
        spec = {"stages": [{"name": "rust-check", "commands": [["cargo", "check"]]}]}
        versions = {"rustc": {"path": "/managed/rustc", "version": "1"}, "cargo": {"path": "/managed/cargo", "version": "1"}}
        first = runner.cache_identities(spec, {"native/bsd/patch": {"sha256": "before"}}, versions, {})
        changed = runner.cache_identities(spec, {"native/bsd/patch": {"sha256": "after"}}, versions, {})
        self.assertEqual(first["cargo_key"], changed["cargo_key"])
        self.assertNotEqual(first["native_key"], changed["native_key"])
        recipe = runner.cache_identities(spec, {"scripts/build-native.py": {"sha256": "recipe"}}, versions, {})
        self.assertEqual(first["cargo_key"], recipe["cargo_key"])
        self.assertNotEqual(first["native_key"], recipe["native_key"])
        other_stage = runner.cache_identities({"stages": [{"name": "rust-tests", "commands": [["cargo", "test"]]}]}, {}, versions, {})
        self.assertEqual(first["cargo_key"], other_stage["cargo_key"])

    def test_compiler_and_relevant_flags_change_both_cache_identities(self):
        spec = {"stages": []}
        versions = {"rustc": {"path": "/managed/rustc", "version": "1"}}
        first = runner.cache_identities(spec, {}, versions, {})
        for updated_versions, environment in (({"rustc": {"path": "/managed/rustc", "version": "2"}}, {}),
                                               (versions, {"RUSTFLAGS": "-C opt-level=1"}),
                                               (versions, {"CARGO_PROFILE_TEST_DEBUG": "0"})):
            changed = runner.cache_identities(spec, {}, updated_versions, environment)
            self.assertNotEqual(first["cargo_key"], changed["cargo_key"])
            self.assertNotEqual(first["native_key"], changed["native_key"])
        command = {"stages": [{"name": "rust-tests", "commands": [["env", "CARGO_PROFILE_TEST_DEBUG=0", "cargo", "test"]]}]}
        changed = runner.cache_identities(command, {}, versions, {})
        self.assertNotEqual(first["cargo_key"], changed["cargo_key"])
        self.assertNotEqual(first["native_key"], changed["native_key"])
        self.assertEqual(first, runner.cache_identities(spec, {}, versions, {"CARGO_BUILD_JOBS": "8"}))

    def test_inventory_has_every_supplied_host(self):
        inventory = json.loads(coordinator.INVENTORY.read_text())
        self.assertEqual(11, len(inventory))
        self.assertEqual("adjutant@buran", inventory["macos"]["ssh"])
        self.assertEqual("system", inventory["openbsd"]["rust"])
        self.assertEqual("system", inventory["dragonflybsd"]["rust"])


if __name__ == "__main__":
    unittest.main()
