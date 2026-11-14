import importlib.util
from pathlib import Path
import tempfile
import subprocess
import unittest
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
        common = BUILD.selected_patches("cryfs", "Linux")
        mac = BUILD.selected_patches("cryfs", "Darwin")
        self.assertEqual(common, ["native/folder/cryfs/fsyncdir.patch"])
        self.assertEqual(mac, common + ["native/folder/cryfs/macos-compiler.patch"])
        compiler = (BUILD.ROOT / mac[-1]).read_text()
        self.assertIn("IdWrapper.h", compiler)
        self.assertNotIn("fuse-t", compiler)
        self.assertNotIn("src/fspp/fuse/CMakeLists.txt", compiler)
        self.assertEqual(BUILD.selected_patches("gocryptfs", "Darwin"), [])


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
            (source / "unit.cc").write_text("patched\n")
            (root / "fix.patch").write_text(git("diff", "--binary"))
            (source / "unit.cc").write_text("original\n")
            cache = root / "cache"
            cache.mkdir()
            with patch.object(BUILD, "ROOT", root), patch.object(BUILD, "PINS", {"fake": (str(source), revision, "v1")}), patch.object(BUILD, "PATCHES", {"fake": ["fix.patch"]}), (root / "build.log").open("wb") as log:
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
