"""Pure packaging checks; no crypto/filesystem backend is executed."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("cryptomator_build", Path(__file__).resolve().parents[1] / "build-cryptomator.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class DependencyLockTests(unittest.TestCase):
    def test_missing_extra_and_modified_jars_fail_closed(self):
        with tempfile.TemporaryDirectory(prefix="lurker-cryptomator-lock-") as directory:
            root = Path(directory)
            modules = root / "mods"
            modules.mkdir()
            jar = modules / "backend.jar"
            jar.write_bytes(b"reviewed artifact")
            (root / "dependencies.lock.json").write_text(json.dumps({"sha256": {jar.name: BUILD.digest(jar)}}))
            with patch.object(BUILD, "SOURCE", root):
                BUILD.verify_dependencies(modules)
                jar.write_bytes(b"changed artifact")
                with self.assertRaises(SystemExit):
                    BUILD.verify_dependencies(modules)
                jar.unlink()
                with self.assertRaises(SystemExit):
                    BUILD.verify_dependencies(modules)
                jar.write_bytes(b"reviewed artifact")
                (modules / "unreviewed.jar").write_bytes(b"unreviewed")
                with self.assertRaises(SystemExit):
                    BUILD.verify_dependencies(modules)


class RuntimeLinkTests(unittest.TestCase):
    def test_reviewed_native_mount_profiles_do_not_fall_back_to_network_or_macfuse(self):
        self.assertEqual(BUILD.MOUNT_PROFILES["Darwin"], {
            "provider": "org.cryptomator.frontend.fuse.mount.FuseTMountProvider",
            "runtime": "fuse-t", "transport": "fskit", "minimum_os": "26.0",
        })
        self.assertEqual(BUILD.MOUNT_PROFILES["Linux"]["transport"], "fuse3")

    def test_traditional_jdk_uses_jmods(self):
        with tempfile.TemporaryDirectory(prefix="lurker-jdk-link-") as directory:
            root = Path(directory)
            (root / "jmods").mkdir()
            (root / "jmods/java.base.jmod").touch()
            self.assertEqual(BUILD.runtime_link_options(root), ["--module-path", str(root / "jmods")])

    def test_linkable_jdk_without_jmods_is_accepted(self):
        with patch.object(BUILD.subprocess, "check_output", return_value="Capabilities:\n  Linking from run-time image enabled\n"):
            self.assertEqual(BUILD.runtime_link_options(Path("/missing-jmods")), [])

    def test_ide_runtime_without_linking_support_is_rejected(self):
        with patch.object(BUILD.subprocess, "check_output", return_value="jlink usage without runtime-image capability"):
            with self.assertRaises(SystemExit):
                BUILD.runtime_link_options(Path("/missing-jmods"))


if __name__ == "__main__":
    unittest.main()
