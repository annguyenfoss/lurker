import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("linux_private_elf", Path(__file__).parents[1] / "linux_private_elf.py")
elf = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(elf)


class PrivateElfTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="lurker-elf-unit-")
        self.root = Path(self.temporary.name)
        self.helper = self.root / "lurker-test"
        self.helper.write_bytes(b"not executed")

    def tearDown(self):
        self.temporary.cleanup()

    @staticmethod
    def info(needed=(), soname=(), search=(), machine="native"):
        return {"needed": list(needed), "soname": list(soname), "search": list(search), "machine": machine}

    def test_recursive_closure_keeps_host_abi_and_deduplicates(self):
        one, two = self.root / "libone.so.1", self.root / "libtwo.so.2"
        graph = {self.helper: self.info(["libone.so.1", "libc.so.6", "libresolv.so.2", "libstdc++.so.6"]),
                 one: self.info(["libtwo.so.2", "libc.so.6"], [one.name]),
                 two: self.info(["libone.so.1", "libgcc_s.so.1"], [two.name])}
        with mock.patch.object(elf, "elf_info", side_effect=graph.__getitem__), \
             mock.patch.object(elf, "system_library", side_effect=lambda name, machine: self.root / name) as resolver:
            self.assertEqual(elf.private_closure([self.helper]), {one.name: one, two.name: two})
        self.assertEqual(resolver.call_count, 2)

    def test_unsafe_producer_rpath_is_rejected_before_resolution(self):
        with mock.patch.object(elf, "elf_info", return_value=self.info(["libfake.so.1"], search=["/tmp/build-cache"])):
            with self.assertRaisesRegex(RuntimeError, "Unsafe/non-private"):
                elf.private_closure([self.helper])

    def test_mixed_architectures_are_rejected(self):
        library = self.root / "libfake.so.1"
        with mock.patch.object(elf, "elf_info", side_effect=[self.info([library.name]), self.info(machine="foreign")]), \
             mock.patch.object(elf, "system_library", return_value=library):
            with self.assertRaisesRegex(RuntimeError, "Mixed ELF architectures"):
                elf.private_closure([self.helper])

    def test_no_soname_abi_aliases(self):
        requested = self.root / "libfuse3.so.3"
        requested.write_bytes(b"not executed")
        with mock.patch.object(elf, "LIBRARY_DIRS", [str(self.root)]), \
             mock.patch.object(elf, "elf_info", return_value=self.info(soname=["libfuse3.so.4"])):
            with self.assertRaisesRegex(RuntimeError, "SONAME differs"):
                elf.system_library(requested.name, "native")

    def test_unresolved_dependency_is_not_silently_omitted(self):
        with mock.patch.object(elf, "LIBRARY_DIRS", [str(self.root)]):
            with self.assertRaisesRegex(RuntimeError, "Cannot resolve"):
                elf.system_library("libnot-installed.so.1", "native")

    def test_absolute_needed_paths_are_rejected(self):
        with self.assertRaisesRegex(RuntimeError, "Invalid dependency"):
            elf.system_library("/tmp/libfake.so", "native")

    def test_glibc_cannot_be_injected_into_private_manifest(self):
        fake = self.root / "libc.so.6"; fake.write_bytes(b"not executed")
        with mock.patch.object(elf, "private_closure", return_value={"libc.so.6": fake}), \
             mock.patch.object(elf, "patcher", return_value=(Path("/not/executed"), {})):
            with self.assertRaisesRegex(RuntimeError, "Forbidden glibc"):
                elf.bundle(self.root, [self.helper.name], self.root / "cache")
        self.assertFalse((self.root / "lib/libc.so.6").exists())

    def test_missing_license_is_a_hard_failure(self):
        package = {"name": "lurker-unit-package-with-no-notices", "version": "1", "licenses": "UnprovidedLicense"}
        with self.assertRaisesRegex(RuntimeError, "notices are missing"):
            elf.library_notices(package, self.root)

    def test_mutated_bootstrap_archive_is_rejected(self):
        cache = self.root / "patchelf-0.18.0"; cache.mkdir()
        (cache / "source.tar.bz2").write_bytes(b"invalid archive")
        with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
            elf.patcher(self.root, offline=True)

    def test_changed_cached_bootstrap_binary_is_rejected(self):
        cache = self.root / "patchelf-0.18.0"; (cache / "source/src").mkdir(parents=True)
        (cache / "source.tar.bz2").write_bytes(b"archive checked by mock")
        (cache / "source/src/patchelf").write_bytes(b"changed cached tool")
        (cache / "build.json").write_text(json.dumps({"archive_sha256": elf.PATCHELF_SHA256, "binary_sha256": "original"}))
        with mock.patch.object(elf, "digest", side_effect=lambda p: elf.PATCHELF_SHA256 if Path(p).name == "source.tar.bz2" else "changed"):
            with self.assertRaisesRegex(RuntimeError, "differs from its receipt"):
                elf.patcher(self.root, offline=True)

    def test_symlink_helpers_are_rejected(self):
        link = self.root / "helper-link"; link.symlink_to(self.helper)
        with self.assertRaisesRegex(RuntimeError, "Unsafe/missing helper"):
            elf.bundle(self.root, [link.name], self.root / "cache")

    @unittest.skipUnless(shutil.which("cc") and shutil.which("readelf"), "ELF build metadata tools unavailable")
    def test_real_native_elf_soname_is_read_without_executing_payload(self):
        source = self.root / "example.c"; source.write_text("int example(void) { return 42; }\n")
        library = self.root / "libexample.so.1"
        subprocess.run(["cc", "-shared", "-fPIC", "-nostdlib", "-Wl,-soname,libexample.so.1", str(source), "-o", str(library)], check=True, capture_output=True)
        info = elf.elf_info(library)
        self.assertEqual(info["soname"], [library.name])
        self.assertEqual(info["needed"], [])
        self.assertEqual(info["search"], [])


if __name__ == "__main__":
    unittest.main()
