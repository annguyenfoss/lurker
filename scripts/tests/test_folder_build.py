import importlib.util
from pathlib import Path
import tempfile
import subprocess
import unittest
from types import SimpleNamespace
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("folder_build", Path(__file__).parents[1] / "build-folder-backends.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class NoticeTests(unittest.TestCase):
    def test_rebuild_replaces_readonly_module_cache_notices(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, output = root / "source", root / "LICENSE"
            source.write_text("updated notice")
            source.chmod(0o444)
            output.write_text("old notice")
            output.chmod(0o444)
            BUILD.copy_notice(source, output)
            BUILD.copy_notice(source, output)
            self.assertEqual(output.read_text(), "updated notice")
            self.assertEqual(output.stat().st_mode & 0o777, 0o644)
            self.assertFalse(list(root.glob(".notice-*")))

    def test_runtime_inventory_retains_vendor_patents_without_build_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            goroot, output = root / "toolchain", root / "notices"
            vendor = goroot / "src/vendor/example.org/crypto"
            vendor.mkdir(parents=True)
            (goroot / "LICENSE").write_text("Go BSD notice")
            (vendor / "PATENTS").write_text("patent grant")
            (vendor / "LICENSE").write_text("dependency notice")
            with patch.object(BUILD, "capture", return_value=str(goroot)):
                inventory = BUILD.go_runtime_notices(root, output)
            self.assertIn("LICENSE", inventory)
            self.assertIn("src/vendor/example.org/crypto/PATENTS", inventory)
            self.assertTrue(all(not Path(path).is_absolute() for path in inventory))


    def test_homebrew_runtime_license_lives_above_libexec(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            goroot = root / "go/libexec"
            (goroot / "src").mkdir(parents=True)
            (goroot.parent / "LICENSE").write_text("Go BSD license")
            with patch.object(BUILD, "capture", return_value=str(goroot)), patch.dict(BUILD.os.environ, {}, clear=True):
                inventory = BUILD.go_runtime_notices(root, root / "notices")
            self.assertIn("LICENSE", inventory)
            self.assertEqual((root / "notices/LICENSE").read_text(), "Go BSD license")


class PlatformPatchTests(unittest.TestCase):
    def test_mac_compiler_patch_preserves_common_durability_and_native_fuse(self):
        common = ["native/folder/cryfs/fsyncdir.patch"]
        linux = BUILD.selected_patches("cryfs", "Linux")
        mac = BUILD.selected_patches("cryfs", "Darwin")
        self.assertEqual(linux, common + ["native/folder/cryfs/linux-localstate.patch"])
        self.assertEqual(mac, common + ["native/folder/cryfs/macos-compiler.patch"])
        compiler = (BUILD.ROOT / mac[-1]).read_text()
        self.assertIn("IdWrapper.h", compiler)
        self.assertNotIn("fuse-t", compiler)
        self.assertNotIn("src/fspp/fuse/CMakeLists.txt", compiler)
        self.assertEqual(BUILD.selected_patches("gocryptfs", "Darwin"), [])


class MacStagingTests(unittest.TestCase):
    def test_private_openmp_license_is_required_and_copied(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "llvm/lib/libomp.dylib"
            library.parent.mkdir(parents=True)
            library.write_bytes(b"OpenMP runtime")
            notice = library.parent.parent / "LICENSE.TXT"
            notice.write_text("LLVM license with exception")
            module = SimpleNamespace(dependencies=lambda _: [str(library)])
            with patch.dict("sys.modules", {"macos_backend": module}):
                BUILD.macos_private_notices(root / "cryfs", {"cryfs-libs/libomp.dylib": "digest"}, root / "licenses")
                self.assertEqual((root / "licenses/llvm-openmp/LICENSE.TXT").read_text(), notice.read_text())
                notice.unlink()
                with self.assertRaisesRegex(RuntimeError, "license missing"):
                    BUILD.macos_private_notices(root / "cryfs", {"cryfs-libs/libomp.dylib": "digest"}, root / "licenses")
                with self.assertRaisesRegex(RuntimeError, "No reviewed"):
                    BUILD.macos_private_notices(root / "cryfs", {"cryfs-libs/unknown.dylib": "digest"}, root / "licenses")

    def test_failed_signing_does_not_publish_private_code(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def stage(binary, destination, libraries, architecture, *, fuse_runtime):
                self.assertEqual(fuse_runtime, "macfuse")
                libraries.mkdir()
                destination.write_bytes(b"executable")
                library = libraries / "libomp.dylib"
                library.write_bytes(b"library")
                return [destination, library], ["/usr/local/lib/libfuse.2.dylib"]
            module = SimpleNamespace(stage_macho=stage, fuse_runtime_info=lambda _: {}, sha256=BUILD.sha256)
            with patch.dict("sys.modules", {"macos_backend": module}), patch.object(
                    BUILD.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "codesign")):
                with self.assertRaises(subprocess.CalledProcessError):
                    BUILD.stage_macos_cryfs(root / "input", root)
            self.assertFalse((root / "cryfs").exists())
            self.assertFalse((root / "cryfs-libs").exists())
            self.assertFalse(list(root.glob(".cryfs-stage-*")))

    def test_existing_macos_artifact_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "cryfs"
            binary.write_bytes(b"previous artifact")
            module = SimpleNamespace(stage_macho=None, fuse_runtime_info=None, sha256=None)
            with patch.dict("sys.modules", {"macos_backend": module}):
                with self.assertRaisesRegex(RuntimeError, "already exists"):
                    BUILD.stage_macos_cryfs(root / "input", root)
            self.assertEqual(binary.read_bytes(), b"previous artifact")


class PatchedSourceTests(unittest.TestCase):
    def make_source(self, root):
        source = root / "source"
        source.mkdir()
        def git(*args):
            return subprocess.check_output(["git", *args], cwd=source, text=True, stderr=subprocess.STDOUT)
        git("init", "-q")
        git("config", "user.name", "Fixture")
        git("config", "user.email", "fixture@example.invalid")
        (source / "unit.cc").write_text("original\n")
        git("add", "unit.cc")
        git("commit", "-qm", "Fixture source")
        revision = git("rev-parse", "HEAD").strip()
        git("tag", "v1")
        return source, revision, git

    def test_patch_worktree_reuse_verifies_diff_and_preserves_tampering(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, revision, git = self.make_source(root)
            (source / "unit.cc").write_text("intermediate\n")
            (root / "first.patch").write_text(git("diff", "--binary"))
            git("add", "unit.cc")
            (source / "unit.cc").write_text("patched\n")
            (root / "fix.patch").write_text(git("diff", "--binary"))
            git("reset", "-q", "HEAD")
            (source / "unit.cc").write_text("original\n")
            cache = root / "cache"
            cache.mkdir()
            with patch.object(BUILD, "ROOT", root), patch.object(BUILD, "PINS", {"fake": (str(source), revision, "v1")}), patch.object(BUILD, "PATCHES", {"fake": ["first.patch", "fix.patch"]}), (root / "build.log").open("wb") as log:
                worktree, identities = BUILD.patched_source("fake", source, cache, log)
                self.assertEqual((source / "unit.cc").read_text(), "original\n")
                self.assertEqual((worktree / "unit.cc").read_text(), "patched\n")
                self.assertEqual(identities["fix.patch"], BUILD.sha256(root / "fix.patch"))
                self.assertEqual(BUILD.patched_source("fake", source, cache, log)[0], worktree)
                (worktree / "unit.cc").write_text("unexpected edit\n")
                with self.assertRaisesRegex(RuntimeError, "Modified or incomplete"):
                    BUILD.patched_source("fake", source, cache, log)
                self.assertEqual((worktree / "unit.cc").read_text(), "unexpected edit\n")

    def test_pristine_cache_rejects_untracked_source_injection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, revision, _ = self.make_source(root)
            cache = root / "cache"
            cache.mkdir()
            with patch.object(BUILD, "PINS", {"fake": (str(source), revision, "v1")}), (root / "build.log").open("wb") as log:
                cached = BUILD.source("fake", cache, log)
                (cached / "injected.cc").write_text("unexpected build input\n")
                with self.assertRaisesRegex(RuntimeError, "Modified fake source cache"):
                    BUILD.source("fake", cache, log)


if __name__ == "__main__":
    unittest.main()
