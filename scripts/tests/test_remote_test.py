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

    def test_inventory_has_every_supplied_host(self):
        inventory = json.loads(coordinator.INVENTORY.read_text())
        self.assertEqual(11, len(inventory))
        self.assertEqual("adjutant@buran", inventory["macos"]["ssh"])
        self.assertEqual("system", inventory["openbsd"]["rust"])
        self.assertEqual("system", inventory["dragonflybsd"]["rust"])


if __name__ == "__main__":
    unittest.main()
