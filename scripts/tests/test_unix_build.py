"""Build orchestration contracts; no compiler or encryption backend executes."""
import argparse
import importlib.util
import hashlib
import json
import os
import platform
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "build-unix.py"
spec = importlib.util.spec_from_file_location("unix_builder", SCRIPT)
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class UnixBuild(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="lurker-build-plan-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.module = builder.stager()
        self.args = argparse.Namespace(source_commit="a" * 40, source_manifest=None,
                                       output=self.root / "package", jobs=2, cli_only=False,
                                       java_home=self.root / "jdk", maven="mvn",
                                       native_keys={"veracrypt": "vc", "folders": "folders", "cryptomator": "java"})

    def recipe(self, system, selected, artifacts=None):
        with mock.patch.dict(builder.os.environ, {}, clear=True):
            return builder.recipe(self.args, system, selected, artifacts or {}, self.root / "cache",
                                  self.root / "work", self.module)

    def test_default_package_builds_both_products_and_records_exact_commit(self):
        stages = dict(self.recipe("NetBSD", {"cgd"}))
        self.assertIn("lurker-desktop", stages["rust-build"])
        self.assertIn("--locked", stages["rust-build"])
        self.assertIn("--desktop", stages["rust-provenance"])
        self.assertIn("--desktop", stages["package"])
        for name in ("rust-provenance", "package"):
            command = stages[name]
            self.assertEqual(command[command.index("--source-commit") + 1], "a" * 40)
        self.assertFalse(any(name.startswith("build-") for name in stages))

    def test_linux_default_veracrypt_includes_userspace_and_creator(self):
        stages = dict(self.recipe("Linux", {"luks", "luks1", "veracrypt", "gocryptfs", "cryfs", "cryptomator"}))
        self.assertNotIn("--only-creator", stages["build-veracrypt-userspace"])
        self.assertIn("userspace-provenance", stages)
        self.assertIn("build-gocryptfs", stages)
        self.assertIn("build-cryfs", stages)
        self.assertIn("build-cryptomator", stages)
        self.assertTrue(any(value.startswith("veracrypt-userspace=") for value in stages["package"]))

    def test_bsd_uses_native_engine_without_linux_builds(self):
        stages = dict(self.recipe("FreeBSD", {"veracrypt", "gocryptfs", "geli", "pefs"}))
        self.assertIn("build-veracrypt-bsd", stages)
        self.assertNotIn("build-veracrypt-userspace", stages)
        self.assertNotIn("build-cryptomator", stages)

    def test_supplied_artifacts_are_verified_by_stager_without_rebuild(self):
        artifact = self.root / "supplied"
        artifact.mkdir()
        supplied = builder.supplied_artifacts(["veracrypt-userspace=" + str(artifact)], {"veracrypt"}, "Linux", self.module)
        stages = dict(self.recipe("Linux", {"veracrypt"}, supplied))
        self.assertNotIn("build-veracrypt-userspace", stages)
        self.assertNotIn("userspace-provenance", stages)
        self.assertIn("veracrypt-userspace=" + str(artifact), stages["package"])

    def test_artifact_alias_and_conflicting_creators_are_rejected(self):
        actual = self.root / "actual"
        actual.mkdir()
        alias = self.root / "alias"
        alias.symlink_to(actual, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "real directory"):
            builder.supplied_artifacts(["gocryptfs=" + str(alias)], {"gocryptfs"}, "Linux", self.module)
        with self.assertRaisesRegex(ValueError, "already includes"):
            builder.supplied_artifacts(["veracrypt-create=" + str(actual), "veracrypt-userspace=" + str(actual)],
                                       {"veracrypt"}, "Linux", self.module)
        with self.assertRaisesRegex(ValueError, "native OS"):
            builder.supplied_artifacts(["veracrypt-bsd=" + str(actual)], {"veracrypt"}, "Linux", self.module)

    def test_missing_java_blocks_before_any_build(self):
        self.args.java_home = None
        with self.assertRaisesRegex(ValueError, "java-home"):
            self.recipe("Linux", {"cryptomator"})

    def test_deployed_source_manifest_is_propagated(self):
        self.args.source_manifest = self.root / "source-manifest.json"
        stages = dict(self.recipe("OpenBSD", {"softraid"}))
        for name in ("rust-provenance", "package"):
            self.assertIn(str(self.args.source_manifest), stages[name])

    def test_manifest_snapshot_rejects_untracked_cargo_configuration(self):
        source = mock.Mock(root=self.root, manifest={"files": {"Cargo.toml": {}}})
        (self.root / "Cargo.toml").write_text("reviewed manifest")
        builder.verify_immutable_source(source)
        (self.root / ".cargo").mkdir()
        (self.root / ".cargo/config.toml").write_text("untracked build override")
        with self.assertRaisesRegex(ValueError, "unexpected/missing inputs"):
            builder.verify_immutable_source(source)

    def test_native_cache_distinguishes_go_cross_targets(self):
        source = mock.Mock(manifest={"files": {}}, commit="a" * 40)
        version = mock.Mock(returncode=0, stdout="same compiler version", stderr="")
        with mock.patch.object(builder.subprocess, "run", return_value=version):
            with mock.patch.dict(builder.os.environ, {"GOOS": "linux", "GOARCH": "amd64"}, clear=True):
                linux = builder.native_key(source, "Linux", [])
            with mock.patch.dict(builder.os.environ, {"GOOS": "freebsd", "GOARCH": "amd64"}, clear=True):
                freebsd = builder.native_key(source, "Linux", [])
        self.assertNotEqual(linux, freebsd)

    def test_cached_userspace_helpers_are_checked_before_rebinding(self):
        directory = self.root / "native"
        directory.mkdir()
        helper = directory / "lurker-fs"
        helper.write_bytes(b"original compiled helper")
        manifest = {"schema": 1, "backend": "veracrypt-userspace", "os": platform.system(),
                    "architecture": platform.machine(), "files": {"lurker-fs": {"sha256": hashlib.sha256(helper.read_bytes()).hexdigest()}}}
        (directory / "artifact-manifest.json").write_text(json.dumps(manifest))
        builder.verify_userspace_cache(directory, self.module)
        helper.write_bytes(b"corrupt cached helper")
        with self.assertRaisesRegex(ValueError, "checksum"):
            builder.verify_userspace_cache(directory, self.module)

    def test_cached_userspace_verification_precedes_rust_build(self):
        directory = self.root / "cache/veracrypt-vc/output"
        directory.mkdir(parents=True)
        (directory / "sources.json").write_text("{}")
        stages = self.recipe("Linux", {"veracrypt"})
        self.assertEqual("userspace-cache-verification", stages[0][0])
        self.assertFalse(any(name == "build-veracrypt-userspace" for name, _ in stages))

    def test_failed_stage_stops_and_retains_source_identity(self):
        work = self.root / "work"
        work.mkdir()
        child = mock.Mock()
        child.wait.return_value = 7
        with mock.patch.object(builder.subprocess, "Popen", return_value=child) as launch:
            result = builder.execute([("compile", ["compiler"]), ("package", ["stager"])], work, 60, "b" * 40)
        self.assertEqual(launch.call_count, 1)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["source_commit"], "b" * 40)
        self.assertTrue((work / "result.json").is_file())
        self.assertFalse((work / "result.json.tmp").exists())

    def test_missing_executable_records_terminal_failure(self):
        work = self.root / "missing-tool"
        work.mkdir()
        with mock.patch.object(builder.subprocess, "Popen", side_effect=FileNotFoundError("missing compiler")):
            result = builder.execute([("compile", ["missing-compiler"])], work, 1, "a" * 40)
        self.assertEqual("failed", result["status"])
        self.assertEqual(127, result["stages"][0]["exit_code"])
        self.assertEqual("failed", json.loads((work / "result.json").read_text())["status"])

    def test_disk_floor_prevents_launch_and_preserves_a_result(self):
        work = self.root / "disk-floor"
        work.mkdir()
        with mock.patch.object(builder.shutil, "disk_usage", return_value=mock.Mock(free=0)), mock.patch.object(builder.subprocess, "Popen") as launch:
            result = builder.execute([("compile", ["compiler"])], work, 1, "a" * 40, minimum_free_bytes=1)
        launch.assert_not_called()
        self.assertEqual("resource-stop", result["stages"][0]["status"])
        self.assertTrue((work / "result.json").is_file())

    def test_real_timeout_terminates_only_the_spawned_build_group(self):
        work = self.root / "timeout"
        work.mkdir()
        result = builder.execute([("compile", [sys.executable, "-c", "import time;time.sleep(60)"])], work, 0.05, "a" * 40)
        self.assertEqual("timeout", result["stages"][0]["status"])
        self.assertEqual(124, result["stages"][0]["exit_code"])

    def test_interrupt_terminates_owned_child_and_records_cancelled(self):
        work = self.root / "interrupted"
        work.mkdir()
        child = mock.Mock(pid=987654321)
        child.poll.return_value = None
        child.wait.side_effect = [KeyboardInterrupt(), 0]
        with mock.patch.object(builder.subprocess, "Popen", return_value=child), mock.patch.object(builder.os, "killpg") as kill:
            result = builder.execute([("compile", ["compiler"])], work, 1, "a" * 40)
        kill.assert_called_once_with(child.pid, builder.signal.SIGTERM)
        self.assertEqual("cancelled", result["status"])
        self.assertEqual(130, result["stages"][0]["exit_code"])

    def test_prerequisites_report_missing_tools_before_expensive_builds(self):
        stages = self.recipe("Linux", {"veracrypt", "cryfs"})
        with mock.patch.object(builder.shutil, "which", return_value=None):
            with self.assertRaisesRegex(ValueError, "missing build prerequisites.*cmake.*wx-config"):
                builder.prerequisites(stages, self.args)

    def test_native_only_build_does_not_require_java_or_cxx(self):
        needed = []
        with mock.patch.object(builder.shutil, "which", side_effect=lambda name: needed.append(name) or "/bin/tool"):
            builder.prerequisites(self.recipe("NetBSD", {"cgd"}), self.args)
        self.assertNotIn("mvn", needed)
        self.assertNotIn("c++", needed)


if __name__ == "__main__":
    unittest.main()
