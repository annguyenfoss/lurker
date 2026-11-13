"""Packaging policy and path tests; no payloads or encryption tools execute."""
import argparse
import importlib.util
import json
import os
import shutil
from pathlib import Path
import stat
import tarfile
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "stage-unix.py"
spec = importlib.util.spec_from_file_location("unix_stager", SCRIPT)
stager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(stager)


class PackagingPolicy(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-stage-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_external_symlink_is_rejected_before_copy(self):
        source = self.root / "source"
        source.mkdir()
        secret = self.root / "private"
        secret.write_text("must not package")
        (source / "escape").symlink_to(secret)
        with self.assertRaisesRegex(ValueError, "external source symlink"):
            stager.copy_tree(source, self.root / "output")
        self.assertFalse((self.root / "output").exists())

    def test_symlinked_root_is_rejected(self):
        actual = self.root / "actual"
        actual.mkdir()
        link = self.root / "link"
        link.symlink_to(actual, target_is_directory=True)
        with self.assertRaisesRegex(ValueError, "real directory"):
            stager.safe_root(link)

    def test_internal_absolute_java_link_becomes_relative_and_preserves_tree(self):
        source = self.root / "java"
        (source / "runtime/lib").mkdir(parents=True)
        (source / "runtime/lib/jvm.so").write_bytes(b"library")
        (source / "runtime/lib/server.so").symlink_to(source / "runtime/lib/jvm.so")
        destination = self.root / "copy"
        stager.copy_tree(source, destination)
        link = destination / "runtime/lib/server.so"
        self.assertEqual("jvm.so", os.readlink(link))
        self.assertEqual(b"library", link.read_bytes())
        self.assertEqual(0o644, stat.S_IMODE((destination / "runtime/lib/jvm.so").stat().st_mode))

    def test_traversal_and_absolute_manifest_paths_are_rejected(self):
        for name in ("../private", "/etc/passwd", "safe/../../private"):
            with self.assertRaisesRegex(ValueError, "unsafe relative path"):
                stager.verify_files(self.root, {name: {"sha256": "bad"}})

    def test_fifo_and_setuid_payloads_are_rejected(self):
        fifo = self.root / "fifo"
        os.mkfifo(fifo)
        with self.assertRaisesRegex(ValueError, "special file"):
            list(stager.entries(self.root))
        fifo.unlink()
        path = self.root / "setuid"
        path.write_text("payload")
        path.chmod(0o4755)
        with self.assertRaisesRegex(ValueError, "setuid"):
            list(stager.entries(self.root))

    def test_wrong_os_and_architecture_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "OS/architecture"):
            stager.verify_platform({"os": "NotThisOS", "architecture": "unknown"})

    def test_checksum_mutation_is_rejected(self):
        payload = self.root / "payload"
        payload.write_bytes(b"original")
        expected = {"payload": {"sha256": stager.digest(payload)}}
        stager.verify_files(self.root, expected)
        payload.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum"):
            stager.verify_files(self.root, expected)

    def test_missing_runtime_does_not_count_as_success(self):
        source = mock.Mock()
        source.commit = "a" * 40
        args = argparse.Namespace(source_root=self.root, source_commit=source.commit, source_manifest=None,
                                  select=["gocryptfs"], backend=[])
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"):
            with self.assertRaisesRegex(ValueError, "runtime missing"):
                stager.stage(args)

    def test_backend_os_pair_must_be_approved(self):
        source = mock.Mock()
        args = argparse.Namespace(source_root=self.root, source_commit="a" * 40, source_manifest=None,
                                  select=["geli"], backend=[])
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"):
            with self.assertRaisesRegex(ValueError, "outside approved scope"):
                stager.stage(args)

    def test_unresolved_library_and_developer_rpath_fail_closed(self):
        binary = self.root / "binary"
        binary.write_bytes(b"not executed")
        for info, message in (({"needed": ["liblurker-impossible.so.999"], "search": [], "interpreters": []}, "unresolved"),
                              ({"needed": [], "search": ["/home/developer/lib"], "interpreters": []}, "untrusted")):
            with mock.patch.object(stager, "elf_info", return_value=info):
                with self.assertRaisesRegex(ValueError, message):
                    stager.audit_dependencies(self.root)

    def test_real_elf_dependency_audit_executes_metadata_tools_only(self):
        binary = self.root / "never-execute-payload"
        shutil.copyfile("/bin/sh", binary)
        original_run = stager.subprocess.run
        def metadata_only(argv, **kwargs):
            self.assertIn(Path(argv[0]).name, {"readelf", "llvm-readelf", "ldconfig"})
            return original_run(argv, **kwargs)
        with mock.patch.object(stager.subprocess, "run", side_effect=metadata_only):
            records = stager.audit_dependencies(self.root)
        self.assertTrue(records)
        self.assertTrue(any(item["bundled"] for item in records))
        self.assertTrue(any(not dependency["bundled"] for item in records for dependency in item["dependencies"]))

    def _stage_fixture(self):
        binaries = self.root / "binaries"
        binaries.mkdir()
        for name in ("lurker", "lurker-helper"):
            (binaries / name).write_bytes(b"\x7fELFfixture-only-never-executed")
            (binaries / name).chmod(0o755)
        provenance = {"schema": 1, "backend": "binaries", "source_commit": "a" * 40, "os": stager.platform.system(),
                      "architecture": stager.platform.machine(), "files": {name: {"sha256": stager.digest(binaries / name)} for name in ("lurker", "lurker-helper")}}
        manifest = self.root / "binaries.json"
        manifest.write_text(json.dumps(provenance))
        source = mock.Mock()
        source.commit = provenance["source_commit"]
        source.read.return_value = b"package license"
        args = argparse.Namespace(source_root=self.root, source_commit=source.commit, source_manifest=None,
                                  select=["luks"], backend=[], binaries=binaries, binary_manifest=manifest,
                                  output=self.root / "package", desktop=False)
        return args, source

    def test_competing_output_creation_is_preserved(self):
        args, source = self._stage_fixture()
        original = Path.symlink_to
        def competing(path, target, target_is_directory=False):
            if path == args.output:
                path.mkdir()
                (path / "owned-by-other-writer").write_text("keep")
            return original(path, target, target_is_directory=target_is_directory)
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/host-tool"), mock.patch.object(stager, "audit_dependencies", return_value=[]), mock.patch.object(Path, "symlink_to", competing):
            with self.assertRaises(FileExistsError):
                stager.stage(args)
        self.assertEqual("keep", (args.output / "owned-by-other-writer").read_text())
        self.assertFalse(list(self.root.glob(".package-*")))

    def test_completed_tree_is_published_atomically_and_tar_has_normal_dirs(self):
        args, source = self._stage_fixture()
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/host-tool"), mock.patch.object(stager, "audit_dependencies", return_value=[]):
            self.assertEqual(0, stager.stage(args))
        self.assertTrue(args.output.is_symlink())
        manifest = json.loads((args.output / "share/lurker/manifest.json").read_text())
        self.assertEqual(["luks"], manifest["selected_backends"])
        self.assertFalse(manifest["self_contained"])
        with tarfile.open(args.output / "package.tar.gz") as archive:
            names = archive.getnames()
            self.assertIn("lurker/bin/lurker", names)
            self.assertNotIn("lurker/package.tar.gz", names)
            self.assertEqual(0, archive.getmember("lurker/bin/lurker").uid)
            self.assertTrue(archive.getmember("lurker/bin").isdir())
        # Existing empty directories must also survive a second publication.
        actual = args.output.resolve()
        args.output.unlink()
        args.output.mkdir()
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/host-tool"):
            with self.assertRaisesRegex(ValueError, "must not exist"):
                stager.stage(args)
        self.assertTrue(actual.is_dir())
        self.assertTrue(args.output.is_dir())


if __name__ == "__main__":
    unittest.main()
