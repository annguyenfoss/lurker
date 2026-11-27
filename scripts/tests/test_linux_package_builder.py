"""Linux package selections retain their actual native/userspace artifact route."""
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
spec = importlib.util.spec_from_file_location("linux_builder", Path(__file__).resolve().parents[1] / "build-linux-package.py")
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)


class NativeSelection(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.prefix = Path(temporary.name)
        (self.prefix / "bin").mkdir()
        for product in builder.PRODUCTS:
            path = self.prefix / "bin" / product
            path.write_bytes(b"test executable")
            path.chmod(0o755)
        self.manifest = {"selected_backends": sorted(builder.REQUIRED_BACKENDS),
                         "bundled_artifacts": sorted(builder.REQUIRED_ARTIFACTS),
                         "desktop_runtime": {"renderer": "software"},
                         "userspace_filesystem": {"bundled": False, "helper": "lurker-fs", "protocol": 5,
                            "contract": "private-crypto-external-filesystem"}}

    def test_accepts_either_verified_veracrypt_route(self):
        for artifact in builder.VERACRYPT_ARTIFACTS:
            with self.subTest(artifact=artifact):
                self.manifest["bundled_artifacts"] = sorted(builder.REQUIRED_ARTIFACTS | {artifact})
                builder.require_full_package(self.prefix, self.manifest)

    def test_rejects_missing_ambiguous_and_unknown_artifacts(self):
        for extra in (set(), builder.VERACRYPT_ARTIFACTS, {"veracrypt-create", "unknown"}):
            with self.subTest(extra=extra):
                self.manifest["bundled_artifacts"] = sorted(builder.REQUIRED_ARTIFACTS | extra)
                with self.assertRaisesRegex(ValueError, "full Linux package"):
                    builder.require_full_package(self.prefix, self.manifest)

    def test_rejects_bundled_filesystem_or_fuse_and_missing_contract(self):
        self.manifest["bundled_artifacts"].append("veracrypt-userspace")
        for name in ("lurker-fs", "lklfuse", "libfuse3.so.4"):
            path = self.prefix / name; path.write_bytes(b"forbidden")
            with self.assertRaisesRegex(ValueError, "must not bundle"):
                builder.require_full_package(self.prefix, self.manifest)
            path.unlink()
        self.manifest["userspace_filesystem"]["protocol"] = 4
        with self.assertRaisesRegex(ValueError, "external host filesystem contract"):
            builder.require_full_package(self.prefix, self.manifest)
        del self.manifest["userspace_filesystem"]
        with self.assertRaisesRegex(ValueError, "external host filesystem contract"):
            builder.require_full_package(self.prefix, self.manifest)

    def test_requires_all_products_and_desktop_runtime(self):
        self.manifest["bundled_artifacts"].append("veracrypt-create")
        self.manifest["desktop_runtime"] = None
        with self.assertRaisesRegex(ValueError, "desktop runtime"):
            builder.require_full_package(self.prefix, self.manifest)
        self.manifest["desktop_runtime"] = {"renderer": "software"}
        (self.prefix / "bin/lurker-helper").unlink()
        with self.assertRaisesRegex(ValueError, "executable"):
            builder.require_full_package(self.prefix, self.manifest)


if __name__ == "__main__":
    unittest.main()
