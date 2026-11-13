"""Tests for source verification and private Mach-O dependency boundaries."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("macos_backend", Path(__file__).parents[1] / "macos_backend.py")
backend = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(backend)


class MacBackendTests(unittest.TestCase):
    def test_external_fuse_allowlist_does_not_allow_homebrew_or_arbitrary_libraries(self):
        for path in ["/usr/local/lib/libfuse-t.dylib", "/usr/local/lib/libfuse-t-1.2.9.dylib"]:
            self.assertTrue(backend.is_external_fuse(path))
        for path in ["/opt/homebrew/lib/libfuse-t.dylib", "/usr/local/lib/libcrypto.dylib",
                     "/tmp/libfuse-t.dylib", "/usr/local/lib/libfuse-t-malicious.dylib"]:
            self.assertFalse(backend.is_external_fuse(path))

    def test_corrupt_cached_download_is_rejected_even_with_existing_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            digest = backend.SOURCES["veracrypt"][1]
            (cache / f"veracrypt-{digest[:16]}.archive").write_bytes(b"corrupt")
            (cache / f"veracrypt-{digest[:16]}").mkdir()
            with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                backend.source("veracrypt", cache, offline=True)

    def test_private_dependency_closure_is_rewritten_and_fuse_stays_external(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, library = root / "backend", root / "libprivate.dylib"
            source.write_bytes(b"exe")
            library.write_bytes(b"lib")
            destination, frameworks = root / "app/Helpers/backend", root / "app/Frameworks"
            changes = []

            def deps(path):
                if path == source:
                    return [str(library), "/usr/local/lib/libfuse-t.dylib", "/usr/lib/libSystem.B.dylib"]
                if path == library:
                    return [str(library)]
                return []

            def run(*arguments, **kwargs):
                changes.append(arguments)
                return ""

            with patch.object(backend, "dependencies", side_effect=deps), \
                 patch.object(backend, "architectures", return_value={"arm64"}), \
                 patch.object(backend, "run", side_effect=run):
                targets, external = backend.stage_macho(source, destination, frameworks, "arm64")
            self.assertEqual(len(targets), 2)
            self.assertEqual(external, ["/usr/local/lib/libfuse-t.dylib"])
            self.assertIn(("install_name_tool", "-change", str(library),
                           "@loader_path/../Frameworks/libprivate.dylib", destination), changes)
            self.assertFalse((frameworks / "libfuse-t.dylib").exists())

    def test_unresolved_rpath_and_missing_architecture_block_staging(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.write_bytes(b"exe")
            with patch.object(backend, "architectures", return_value={"arm64"}), \
                 patch.object(backend, "dependencies", return_value=["@rpath/libunknown.dylib"]):
                with self.assertRaisesRegex(RuntimeError, "Unresolved dependency"):
                    backend.stage_macho(source, root / "out/backend", root / "out/Frameworks", "arm64")
            with patch.object(backend, "architectures", return_value={"x86_64"}):
                with self.assertRaisesRegex(RuntimeError, "Missing arm64 slice"):
                    backend.stage_macho(source, root / "other/backend", root / "other/Frameworks", "arm64")

    def test_legacy_deployment_target_does_not_use_dylib_compatibility_version(self):
        with patch.object(backend, "run", side_effect=["no minos", """
            cmd LC_ID_DYLIB
            compatibility version 999.0.0
            cmd LC_VERSION_MIN_MACOSX
            cmdsize 16
            version 10.15
            sdk 27.0
        """]):
            self.assertEqual(backend.minimum_version(Path("library.dylib")), "10.15")

    def test_fuse_rpath_is_converted_to_explicit_external_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.write_bytes(b"exe")
            destination = root / "output/backend"
            calls = []
            with patch.object(backend, "architectures", return_value={"arm64"}), \
                 patch.object(backend, "dependencies", side_effect=[["@rpath/libfuse-t.dylib"], []]), \
                 patch.object(Path, "is_file", return_value=True), \
                 patch.object(backend, "run", side_effect=lambda *a, **kw: calls.append(a) or ""):
                _, external = backend.stage_macho(source, destination, root / "output/Frameworks", "arm64")
            self.assertEqual(external, ["/usr/local/lib/libfuse-t.dylib"])
            self.assertIn(("install_name_tool", "-change", "@rpath/libfuse-t.dylib",
                           "/usr/local/lib/libfuse-t.dylib", destination), calls)


if __name__ == "__main__":
    unittest.main()
