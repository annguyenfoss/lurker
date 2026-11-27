import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest import mock

SCRIPT = Path(__file__).resolve().parents[1] / "build-macos-ntfs2btrfs.py"
SPEC = importlib.util.spec_from_file_location("build_macos_ntfs2btrfs", SCRIPT)
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


def archive_members(names):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for name, kind in names:
            item = tarfile.TarInfo(name)
            if kind == "symlink":
                item.type = tarfile.SYMTYPE
                item.linkname = "/etc/passwd"
                archive.addfile(item)
            else:
                data = b"native source\n"
                item.size = len(data)
                archive.addfile(item, io.BytesIO(data))
    buffer.seek(0)
    return buffer


class NativeFormatterBuildInputs(unittest.TestCase):
    def test_patch_digest_is_the_reviewed_input(self):
        self.assertEqual(BUILD.digest(BUILD.PATCH), BUILD.PATCH_SHA256)

    def test_corrupt_archive_is_rejected_and_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.tar.gz"
            path.write_bytes(b"unexpected bytes")
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                BUILD.require_digest(path, BUILD.ARCHIVE_SHA256)
            self.assertEqual(path.read_bytes(), b"unexpected bytes")

    def test_safe_archive_accepts_pinned_prefix_only(self):
        prefix = "ntfs2btrfs-" + BUILD.COMMIT
        with tarfile.open(fileobj=archive_members([(prefix + "/source", "file")])) as archive:
            self.assertEqual(BUILD.safe_members(archive)[0][1], "source")
        for name in ("other/source", prefix + "/../outside", "/" + prefix + "/source",
                     prefix + "//source", prefix + "/./source"):
            with self.subTest(name=name), tarfile.open(fileobj=archive_members([(name, "file")])) as archive:
                with self.assertRaisesRegex(ValueError, "Unsafe"):
                    BUILD.safe_members(archive)

    def test_links_and_duplicate_members_fail_before_extraction(self):
        prefix = "ntfs2btrfs-" + BUILD.COMMIT
        for members in ([(prefix + "/source", "symlink")],
                        [(prefix + "/source", "file"), (prefix + "/source", "file")]):
            with self.subTest(members=members), tarfile.open(fileobj=archive_members(members)) as archive:
                with self.assertRaises(ValueError):
                    BUILD.safe_members(archive)

    def test_invalid_archive_does_not_publish_partial_source(self):
        prefix = "ntfs2btrfs-" + BUILD.COMMIT
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "source.tar.gz"
            archive.write_bytes(archive_members([(prefix + "/source", "file"),
                                                (prefix + "/../outside", "file")]).getvalue())
            with self.assertRaises(ValueError):
                BUILD.extract_source(archive, root / "source")
            self.assertFalse((root / "source").exists())
            self.assertFalse((root / "outside").exists())

    def test_extraction_preserves_an_existing_destination(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            marker = source / "marker"
            marker.write_text("retained")
            with self.assertRaisesRegex(ValueError, "must be new"):
                BUILD.extract_source(root / "unused.tar.gz", source)
            self.assertEqual(marker.read_text(), "retained")

    def test_patch_cannot_add_product_or_driver_changes(self):
        lines = "\n".join("1\t1\t" + name for name in sorted(BUILD.PATCH_FILES))
        BUILD.validate_patch_paths(lines)
        for unexpected in (lines + "\n1\t1\tcrates/lurker-core/src/macos.rs",
                           lines + "\n1\t1\t" + sorted(BUILD.PATCH_FILES)[0],
                           "1\t1\tsrc/crc32c.cpp"):
            with self.assertRaisesRegex(ValueError, "reviewed converter files"):
                BUILD.validate_patch_paths(unexpected)

    def test_output_cannot_be_application_bundle_or_existing_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            root.chmod(0o700)
            with self.assertRaisesRegex(ValueError, "application bundle"):
                BUILD.prepare_output(root / "Lurker.app")
            BUILD.prepare_output(root / "candidate")
            with self.assertRaises(FileExistsError):
                BUILD.prepare_output(root / "candidate")
            self.assertEqual((root / "candidate").stat().st_mode & 0o777, 0o700)
            bundle = root / "Existing.app"
            bundle.mkdir(mode=0o700)
            (root / "alias").symlink_to(bundle, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "application bundle"):
                BUILD.prepare_output(root / "alias" / "candidate")

    def test_policy_checks_only_pass_absent_private_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            calls = []
            def run(command, label, *, accepted):
                calls.append(command)
                self.assertEqual(accepted, (1,))
                self.assertEqual(Path(command[-1]).parent, root)
                self.assertFalse(Path(command[-1]).exists())
                return "Darwin candidate requires --fresh-format=DEVICE:INODE:BYTES"
            results = BUILD.check_policy_refusals(root / "binary", root, run)
            self.assertEqual(len(calls), 4)
            self.assertEqual(len(results), 4)

    def test_policy_checks_refuse_an_existing_or_symlink_sentinel(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            sentinel = root / "private-never-created-policy-sentinel.ntfs"
            for kind in ("file", "symlink"):
                if kind == "file":
                    sentinel.write_text("do not access")
                else:
                    sentinel.symlink_to(root / "absent-target")
                with self.subTest(kind=kind), mock.patch.object(BUILD.subprocess, "run") as run:
                    with self.assertRaisesRegex(RuntimeError, "nonexistent private"):
                        BUILD.check_policy_refusals(root / "binary", root, run)
                    run.assert_not_called()
                sentinel.unlink()

    def test_wrong_host_rejects_before_inputs_or_commands(self):
        for system, machine in (("Linux", "arm64"), ("Darwin", "x86_64")):
            with self.subTest(system=system, machine=machine), \
                    mock.patch.object(BUILD.platform, "system", return_value=system), \
                    mock.patch.object(BUILD.platform, "machine", return_value=machine), \
                    mock.patch.object(BUILD, "require_digest") as digest, \
                    mock.patch.object(BUILD.subprocess, "run") as run:
                with self.assertRaises(SystemExit):
                    BUILD.main(["--source-archive", "/unused", "--llvm-root", "/unused",
                                "--cmake", "/unused", "--ninja", "/unused", "--pkg-config", "/unused",
                                "--output-dir", "/unused"])
                digest.assert_not_called()
                run.assert_not_called()



if __name__ == "__main__":
    unittest.main()
