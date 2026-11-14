"""Tests for source verification and private Mach-O dependency boundaries."""
import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import sys
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("macos_backend", Path(__file__).parents[1] / "macos_backend.py")
backend = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(backend)


class MacBackendTests(unittest.TestCase):
    def test_external_fuse_allowlist_does_not_allow_homebrew_or_arbitrary_libraries(self):
        for path in ["/usr/local/lib/libfuse-t.dylib", "/usr/local/lib/libfuse-t-1.2.9.dylib",
                     "/usr/local/lib/libfuse.2.dylib", "/usr/local/lib/libosxfuse.2.dylib"]:
            self.assertTrue(backend.is_external_fuse(path))
        for path in ["/opt/homebrew/lib/libfuse-t.dylib", "/usr/local/lib/libcrypto.dylib",
                     "/tmp/libfuse-t.dylib", "/usr/local/lib/libfuse-t-malicious.dylib"]:
            self.assertFalse(backend.is_external_fuse(path))

    def test_runtime_flags_and_manifest_selection_do_not_silently_swap_fuse(self):
        self.assertNotIn("flags", backend.FUSE_RUNTIMES["macfuse"])
        self.assertEqual(backend.FUSE_RUNTIMES["fuse-t"]["flags"], ["WITHFUSET=1", "VC_OSX_FUSET=1"])
        self.assertEqual(backend.manifest_fuse_runtime({"fuse_runtime": {"implementation": "macfuse"}}), "macfuse")
        self.assertEqual(backend.manifest_fuse_runtime({"external_libraries": ["/usr/local/lib/libfuse-t.dylib"]}), "fuse-t")
        with self.assertRaisesRegex(RuntimeError, "one supported"):
            backend.manifest_fuse_runtime({"external_libraries": ["/usr/local/lib/libfuse-t.dylib", "/usr/local/lib/libfuse.2.dylib"]})

    def test_veracrypt_manifest_rejects_macfuse_even_though_folder_staging_supports_it(self):
        self.assertEqual(backend.veracrypt_fuse_runtime({"fuse_runtime": {"implementation": "fuse-t"}}), "fuse-t")
        for manifest in [{"fuse_runtime": {"implementation": "macfuse"}},
                         {"external_libraries": ["/usr/local/lib/libfuse.2.dylib"]}]:
            with self.assertRaisesRegex(RuntimeError, "VeraCrypt requires FUSE-T"):
                backend.veracrypt_fuse_runtime(manifest)

    def test_veracrypt_package_rejects_stale_pins_and_escaping_artifacts(self):
        import copy
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "lurker-veracrypt").write_bytes(b"private")
            patch_name = "native/veracrypt/utf8-console.patch"
            project_root = Path(__file__).parents[2]
            manifest = {"fuse_runtime": {"implementation": "fuse-t"},
                        "veracrypt_revision": backend.VC_REVISION,
                        "source_archives": {n: {"url": s[0], "sha256": s[1]} for n, s in backend.SOURCES.items()},
                        "patches": {patch_name: backend.sha256(project_root / patch_name)},
                        "artifacts": {"lurker-veracrypt": backend.sha256(root / "lurker-veracrypt")}}
            self.assertEqual(backend.verify_veracrypt_manifest(root, manifest, project_root), "fuse-t")
            old = copy.deepcopy(manifest); old["veracrypt_revision"] = "old"
            with self.assertRaisesRegex(RuntimeError, "reviewed source/patch"):
                backend.verify_veracrypt_manifest(root, old, project_root)
            escaped = copy.deepcopy(manifest); escaped["artifacts"]["../outside"] = "unused"
            with self.assertRaisesRegex(RuntimeError, "provenance mismatch"):
                backend.verify_veracrypt_manifest(root, escaped, project_root)

    def test_folder_package_binds_darwin_patches_private_libraries_and_notices(self):
        import copy
        project = Path(__file__).parents[2]
        recipe = "scripts/build-folder-backends.py"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for relative in ["cryfs", "cryfs-libs/libomp.dylib", "licenses/cryfs/LICENSE.txt", "licenses/cryfs/native/llvm-openmp/LICENSE.TXT"]:
                path = root / relative; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b"fixture")
            url, revision, _ = backend.source_constant(project, recipe, "PINS")["cryfs"]
            paths = backend.source_constant(project, recipe, "PATCHES")["cryfs"]
            paths += backend.source_constant(project, recipe, "PLATFORM_PATCHES")["Darwin"]["cryfs"]
            manifest = {"source": url, "revision": revision, "target": {"os": "Darwin", "machine": "arm64"},
                        "patches": {r: backend.sha256(project / r) for r in paths},
                        "sha256": backend.sha256(root / "cryfs"),
                        "private_libraries": {"cryfs-libs/libomp.dylib": backend.sha256(root / "cryfs-libs/libomp.dylib")},
                        "license_files": {"LICENSE.txt": backend.sha256(root / "licenses/cryfs/LICENSE.txt"),
                                          "native/llvm-openmp/LICENSE.TXT": backend.sha256(root / "licenses/cryfs/native/llvm-openmp/LICENSE.TXT")},
                        "fuse_runtime": {"implementation": "macfuse"}}
            backend.verify_folder_manifest(root, "cryfs", manifest, project, "arm64")
            missing_notice = copy.deepcopy(manifest)
            del missing_notice["license_files"]["native/llvm-openmp/LICENSE.TXT"]
            with self.assertRaisesRegex(RuntimeError, "LLVM OpenMP notice"):
                backend.verify_folder_manifest(root, "cryfs", missing_notice, project, "arm64")
            old = copy.deepcopy(manifest); del old["patches"][paths[-1]]
            with self.assertRaisesRegex(RuntimeError, "source/patch pins"):
                backend.verify_folder_manifest(root, "cryfs", old, project, "arm64")
            (root / "cryfs-libs/libomp.dylib").write_bytes(b"changed")
            with self.assertRaisesRegex(RuntimeError, "provenance mismatch"):
                backend.verify_folder_manifest(root, "cryfs", manifest, project, "arm64")

    def test_cryptomator_package_rejects_old_helper_even_with_current_dependency_lock(self):
        import json
        project = Path(__file__).parents[2]
        source = project / "native/folder/cryptomator"
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); files = {}
            for relative in ["pom.xml", "src/main/java/module-info.java", "src/main/java/org/lurker/cryptomator/Main.java"]:
                path = root / "source" / relative
                path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes((source / relative).read_bytes())
                files["source/" + relative] = backend.sha256(path)
            manifest = {"os": "Darwin", "architecture": "arm64", "files": files,
                        "sources": json.loads((source / "dependencies.lock.json").read_text())}
            backend.verify_cryptomator_manifest(root, manifest, project, "arm64")
            files["source/src/main/java/org/lurker/cryptomator/Main.java"] = "old"
            with self.assertRaisesRegex(RuntimeError, "helper source differs"):
                backend.verify_cryptomator_manifest(root, manifest, project, "arm64")

    def test_hardened_runtime_exceptions_are_limited_to_external_fuse_consumers(self):
        spec = importlib.util.spec_from_file_location("macos_dmg", Path(__file__).parents[1] / "build-macos-dmg.py")
        dmg = importlib.util.module_from_spec(spec)
        with patch.dict(sys.modules, {"macos_backend": backend}):
            spec.loader.exec_module(dmg)
        helpers = Path("App/Contents/Helpers")
        native, java = Path("native.plist"), Path("java.plist")
        jre = Path("App/Contents/Resources/runtime/bin/java")
        for name in ["lurker-veracrypt", "cryfs"]:
            self.assertEqual(dmg.signing_entitlements(helpers / name, helpers, [jre], native, java), ["--entitlements", native])
        self.assertEqual(dmg.signing_entitlements(jre, helpers, [jre], native, java), ["--entitlements", java])
        for path in [helpers / "gocryptfs", Path("App/Contents/MacOS/lurker"), Path("App/Contents/MacOS/lurker-desktop")]:
            self.assertEqual(dmg.signing_entitlements(path, helpers, [jre], native, java), [])

    def test_external_runtime_rejects_user_writable_ancestors(self):
        with tempfile.TemporaryDirectory() as temporary:
            library = Path(temporary) / "library.dylib"
            library.write_bytes(b"library")
            with self.assertRaisesRegex(RuntimeError, "administrator-controlled"):
                backend.require_administrator_controlled(library)

    def test_corrupt_cached_download_is_rejected_even_with_existing_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            digest = backend.SOURCES["veracrypt"][1]
            (cache / f"veracrypt-{digest[:16]}.archive").write_bytes(b"corrupt")
            (cache / f"veracrypt-{digest[:16]}").mkdir()
            with self.assertRaisesRegex(RuntimeError, "checksum mismatch"):
                backend.source("veracrypt", cache, offline=True)

    def test_approved_patch_has_a_separate_verified_source_cache(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            archive = cache / "fixture.tar"
            with tarfile.open(archive, "w") as stream:
                info = tarfile.TarInfo("project/input.txt")
                contents = b"original\n"
                info.size = len(contents)
                stream.addfile(info, io.BytesIO(contents))
            digest = backend.sha256(archive)
            archive.rename(cache / f"fixture-{digest[:16]}.archive")
            approved_patch = cache / "approved.patch"
            approved_patch.write_text("--- a/input.txt\n+++ b/input.txt\n@@ -1 +1 @@\n-original\n+approved\n")
            with patch.dict(backend.SOURCES, {"fixture": ("unused", digest)}):
                original = backend.source("fixture", cache, offline=True)
                modified = backend.source("fixture", cache, offline=True, patch=approved_patch)
                self.assertNotEqual(original, modified)
                self.assertEqual((original / "input.txt").read_text(), "original\n")
                self.assertEqual((modified / "input.txt").read_text(), "approved\n")
                (modified / "input.txt").write_text("unapproved\n")
                with self.assertRaisesRegex(RuntimeError, "source input changed"):
                    backend.source("fixture", cache, offline=True, patch=approved_patch)

    def test_runtime_source_caches_keep_distinct_native_object_trees(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            archive = cache / "fixture.tar"
            with tarfile.open(archive, "w") as stream:
                info = tarfile.TarInfo("project/input.txt"); info.size = 4
                stream.addfile(info, io.BytesIO(b"same"))
            digest = backend.sha256(archive)
            archive.rename(cache / f"fixture-{digest[:16]}.archive")
            with patch.dict(backend.SOURCES, {"fixture": ("unused", digest)}):
                a = backend.source("fixture", cache, offline=True, variant="macfuse")
                b = backend.source("fixture", cache, offline=True, variant="fuse-t")
                self.assertNotEqual(a, b)
                (a / "generated.o").write_bytes(b"native objects")
                self.assertFalse((b / "generated.o").exists())

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
                 patch.object(backend, "require_administrator_controlled"), \
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
                 patch.object(backend, "require_administrator_controlled"), \
                 patch.object(backend, "run", side_effect=lambda *a, **kw: calls.append(a) or ""):
                _, external = backend.stage_macho(source, destination, root / "output/Frameworks", "arm64")
            self.assertEqual(external, ["/usr/local/lib/libfuse-t.dylib"])
            self.assertIn(("install_name_tool", "-change", "@rpath/libfuse-t.dylib",
                           "/usr/local/lib/libfuse-t.dylib", destination), calls)

    def test_wrong_runtime_dependency_blocks_staging_instead_of_copying_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); source = root / "source"; source.write_bytes(b"exe")
            with patch.object(backend, "architectures", return_value={"arm64"}), \
                 patch.object(backend, "dependencies", return_value=["/usr/local/lib/libfuse-t.dylib"]):
                with self.assertRaisesRegex(RuntimeError, "differs from selected runtime"):
                    backend.stage_macho(source, root / "out/backend", root / "out/Frameworks", "arm64", fuse_runtime="macfuse")

    def test_macfuse_stays_external_and_is_not_added_to_private_frameworks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); source = root / "source"; source.write_bytes(b"exe")
            with patch.object(backend, "architectures", return_value={"arm64"}), \
                 patch.object(backend, "dependencies", side_effect=[["/usr/local/lib/libfuse.2.dylib"], []]), \
                 patch.object(backend, "require_administrator_controlled") as trusted, \
                 patch.object(backend, "run", return_value=""):
                targets, external = backend.stage_macho(source, root / "out/backend", root / "out/Frameworks", "arm64", fuse_runtime="macfuse")
            trusted.assert_called_once_with("/usr/local/lib/libfuse.2.dylib")
            self.assertEqual(external, ["/usr/local/lib/libfuse.2.dylib"])
            self.assertEqual(len(targets), 1)
            self.assertFalse((root / "out/Frameworks/libfuse.2.dylib").exists())


if __name__ == "__main__":
    unittest.main()
