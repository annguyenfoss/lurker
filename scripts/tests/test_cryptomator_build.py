"""Pure packaging checks; no crypto/filesystem backend is executed."""
import importlib.util
import json
import difflib
import subprocess
from pathlib import Path
import tempfile
import unittest
import zipfile
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


class LinuxPatchTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-java-patch-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "linux-patches").mkdir()
        self.entry = {"module": "org.cryptomator.cryptofs",
                      "artifact": "org.cryptomator:cryptofs:2.8.0:jar:sources",
                      "base_jar": "cryptofs-2.8.0.jar", "source_sha256": "a" * 64,
                      "patch": "cryptofs-linux.patch", "jar": "cryptofs-linux.jar"}

    def save(self, modules):
        (self.root / "linux-patches/manifest.json").write_text(json.dumps({"schema": 1, "modules": modules}))

    def test_patch_configuration_pins_module_and_rejects_injected_paths(self):
        self.save([self.entry])
        self.assertEqual([self.entry], BUILD.linux_patch_configuration(self.root))
        for field, value in (("module", "unreviewed.module"), ("artifact", "other:cryptofs:3:jar:sources"),
                             ("base_jar", "other.jar"), ("source_sha256", "missing"),
                             ("patch", "../outside.patch"), ("jar", "/outside.jar")):
            with self.subTest(field=field):
                self.save([{**self.entry, field: value}])
                with self.assertRaises(SystemExit):
                    BUILD.linux_patch_configuration(self.root)
        self.save([self.entry, self.entry])
        with self.assertRaises(SystemExit):
            BUILD.linux_patch_configuration(self.root)

    def test_patch_applies_to_verified_java_sources_and_lists_changed_units(self):
        archive = self.root / "upstream.jar"
        name = "org/example/Storage.java"
        old = "package org.example;\nclass Storage { int value = 1; }\n"
        new = old.replace("= 1", "= 2")
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr(name, old.replace("\n", "\r\n"))
        patch = self.root / "change.patch"
        patch.write_text("".join(difflib.unified_diff(old.splitlines(keepends=True), new.splitlines(keepends=True),
                                                  fromfile="a/" + name, tofile="b/" + name)))
        destination = self.root / "patched"
        destination.mkdir()
        self.assertEqual([name], BUILD.patch_java_sources(archive, patch, destination))
        self.assertEqual(new, (destination / name).read_text())

    def test_patch_cannot_modify_files_outside_java_tree_or_module_descriptor(self):
        archive = self.root / "upstream.jar"
        with zipfile.ZipFile(archive, "w"):
            pass
        patch = self.root / "bad.patch"
        for name in ("../../outside.java", "/outside.java", "a/module-info.java", "a/runtime.sh"):
            patch.write_text("--- /dev/null\n+++ " + name + "\n")
            with self.subTest(name=name), self.assertRaises(SystemExit):
                BUILD.patch_java_sources(archive, patch, self.root)

    def test_already_applied_patch_is_rejected_instead_of_reversed(self):
        name = "org/example/Storage.java"
        old, new = "class Storage { int value = 1; }\n", "class Storage { int value = 2; }\n"
        archive = self.root / "upstream.jar"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr(name, new)
        patch = self.root / "change.patch"
        patch.write_text("".join(difflib.unified_diff(old.splitlines(keepends=True), new.splitlines(keepends=True),
                                                  fromfile="a/" + name, tofile="b/" + name)))
        destination = self.root / "patched"
        destination.mkdir()
        with self.assertRaises(subprocess.CalledProcessError):
            BUILD.patch_java_sources(archive, patch, destination)
        self.assertEqual(new, (destination / name).read_text())

    def test_runtime_patch_options_use_only_reviewed_access_and_private_paths(self):
        options = BUILD.linux_patch_options(Path("/private/runtime"), [self.entry, {
            "module": "org.cryptomator.frontend.fuse", "jar": "fuse-nio-linux.jar",
        }])
        self.assertIn("org.cryptomator.cryptofs=/private/runtime/patches/cryptofs-linux.jar", options)
        self.assertIn("org.cryptomator.frontend.fuse=/private/runtime/patches/fuse-nio-linux.jar", options)
        self.assertIn("java.base/sun.nio.ch=org.cryptomator.cryptofs", options)
        self.assertIn("org.cryptomator.cryptofs/org.cryptomator.cryptofs.fh=org.cryptomator.frontend.fuse", options)


if __name__ == "__main__":
    unittest.main()
