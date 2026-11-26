import importlib.util
import json
from pathlib import Path
import tempfile
import subprocess
import unittest
from types import SimpleNamespace
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("folder_build", Path(__file__).parents[1] / "build-folder-backends.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class ConanTests(unittest.TestCase):
    def test_cryfs_uses_boost_headers_from_the_locked_host_graph(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / "host-package"
            header = package / "include/boost/version.hpp"
            header.parent.mkdir(parents=True)
            header.touch()
            options = ["--lockfile", "test.lock"]

            def write_graph(args, cwd, log, env):
                self.assertEqual(args[-2:], options)
                target = Path(args[args.index("--out-file") + 1])
                target.write_text(json.dumps({"graph": {"nodes": {
                    "1": {"context": "host", "ref": "boost/1.84.0#fixture",
                          "package_folder": str(package)},
                    "2": {"context": "build", "ref": "boost/1.84.0#fixture",
                          "package_folder": str(root / "build-package")},
                }}}))

            with patch.object(BUILD, "run", side_effect=write_graph):
                path, prefix = BUILD.conan_boost_include(["conan"], options, root, root, None, {})
            self.assertEqual(path, header.parent.parent.resolve())
            self.assertEqual(prefix, "/usr/src/boost-1.84.0/include")


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
    def test_mac_fuse_t_patch_preserves_common_durability_and_compiler_fixes(self):
        common = ["native/folder/cryfs/fsyncdir.patch"]
        linux = BUILD.selected_patches("cryfs", "Linux")
        mac = BUILD.selected_patches("cryfs", "Darwin")
        self.assertEqual(linux, common + ["native/folder/cryfs/linux-localstate.patch",
                                         "native/folder/cryfs/linux-exclusive-publication.patch"])
        self.assertEqual(mac, common + ["native/macos/cryfs-fuse-t.patch",
                                       "native/folder/cryfs/macos-exclusive-publication.patch"])
        compiler = (BUILD.ROOT / "native/macos/cryfs-fuse-t.patch").read_text()
        self.assertIn("IdWrapper.h", compiler)
        self.assertIn("fuse-t", compiler)
        self.assertIn("src/fspp/fuse/CMakeLists.txt", compiler)
        self.assertEqual(BUILD.selected_patches("gocryptfs", "Darwin"), [])

    def test_freebsd_preserves_the_common_cryfs_durability_patch(self):
        self.assertEqual(BUILD.selected_patches("cryfs", "FreeBSD"),
                         ["native/folder/cryfs/fsyncdir.patch"])


class FreeBSDRuntimeTests(unittest.TestCase):
    def test_missing_native_library_or_package_notice_blocks_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "cryfs"
            linked = "libfuse.so.2 => /usr/local/lib/libfuse.so.2 (0x123)"
            outputs = {("ldd", str(binary)): linked,
                       ("pkg", "which", "-q", "/usr/local/lib/libfuse.so.2"): "fusefs-libs-2.9.9",
                       ("pkg", "query", "%n %v %o", "fusefs-libs-2.9.9"): "fusefs-libs 2.9.9 sysutils/fusefs-libs",
                       ("pkg", "query", "%L", "fusefs-libs-2.9.9"): "LGPL21"}
            with patch.object(BUILD, "capture", side_effect=lambda argv, _: outputs[tuple(argv)]), \
                    patch.object(BUILD, "FREEBSD_LICENSE_ROOT", root / "native-licenses"):
                with self.assertRaisesRegex(RuntimeError, "package license missing"):
                    BUILD.freebsd_runtime_packages(binary, root, root / "notices")
                license_file = root / "native-licenses/fusefs-libs-2.9.9/LICENSE"
                license_file.parent.mkdir(parents=True)
                license_file.write_text("native dependency notice")
                observed, packages = BUILD.freebsd_runtime_packages(binary, root, root / "notices")
                self.assertEqual(observed, linked)
                self.assertEqual(packages[0]["origin"], "sysutils/fusefs-libs")
                self.assertEqual((root / "notices/fusefs-libs-2.9.9/LICENSE").read_text(),
                                 "native dependency notice")
            with patch.object(BUILD, "capture", return_value="libfuse.so.2 => not found"):
                with self.assertRaisesRegex(RuntimeError, "unresolved native runtime"):
                    BUILD.freebsd_runtime_packages(binary, root, root / "notices")


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
                self.assertEqual(fuse_runtime, "fuse-t")
                libraries.mkdir()
                destination.write_bytes(b"executable")
                library = libraries / "libomp.dylib"
                library.write_bytes(b"library")
                return [destination, library], ["/usr/local/lib/libfuse-t.dylib"]
            module = SimpleNamespace(FOLDER_FUSE_MOUNT={}, stage_macho=stage, fuse_runtime_info=lambda _: {}, sha256=BUILD.sha256)
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
            module = SimpleNamespace(FOLDER_FUSE_MOUNT={}, stage_macho=None, fuse_runtime_info=None, sha256=None)
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


class NetBSDNativeTests(unittest.TestCase):
    def test_base_refuse_headers_precede_pkgsrc_and_reject_compatibility_entrypoints(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            include = root / "base-include"
            include.mkdir()
            for name in ("fuse.h", "fuse_opt.h"):
                (include / name).write_text("/* base ReFUSE fixture */\n")
            env, provenance = BUILD.netbsd_refuse_environment(root / "build", {
                "PKG_CONFIG_PATH": "/existing/pkgconfig", "LDFLAGS": "-Wl,-zrelro"}, include)
            self.assertTrue(env["PKG_CONFIG_PATH"].startswith(str(root / "build/refuse-pkgconfig") + ":"))
            pc = (root / "build/refuse-pkgconfig/fuse.pc").read_text()
            self.assertIn("Cflags: -Wp,-I,", pc)
            self.assertIn("-lrefuse -lpuffs -lutil", pc)
            self.assertNotIn("-lfuse", pc)
            self.assertIn("/usr/pkg/lib", env["LDFLAGS"])
            for name in ("fuse.h", "fuse_opt.h"):
                self.assertEqual((root / "build/refuse-include" / name).read_text(), '#include "' + str(include / name) + '"\n')
            self.assertEqual(2, len(provenance["headers"]))
        BUILD.netbsd_refuse_symbols(" U __fuse_main\n U fuse_get_context\n")
        for symbols in (" U fuse_main_real_v26\n", " U __fuse_main\n U fuse_main_real\n"):
            with self.assertRaisesRegex(RuntimeError, "base ReFUSE API29"):
                BUILD.netbsd_refuse_symbols(symbols)

    def test_native_notice_cache_preserves_reviewed_bytes_and_rejects_mutation(self):
        import hashlib
        import io
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = b"native dependency copyright and license\n"
            checksum = hashlib.sha256(data).hexdigest()
            with patch.dict(BUILD.NETBSD_NOTICE_INPUTS, {"fixture": ("https://example.invalid/LICENSE", checksum)}), patch.object(BUILD.urllib.request, "urlopen", return_value=io.BytesIO(data)) as download:
                result = BUILD.pinned_netbsd_notice("fixture", root / "cache", root / "notices/LICENSE")
                self.assertEqual(data, (root / "notices/LICENSE").read_bytes())
                self.assertEqual(checksum, result["sha256"])
                BUILD.pinned_netbsd_notice("fixture", root / "cache", root / "notices/LICENSE")
                self.assertEqual(1, download.call_count)
                (root / "cache" / (checksum + ".txt")).write_bytes(b"changed")
                with self.assertRaisesRegex(RuntimeError, "cache differs"):
                    BUILD.pinned_netbsd_notice("fixture", root / "cache", root / "notices/LICENSE")

    def test_native_closure_notices_include_header_only_packages_without_bundling_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            include = root / "include"
            include.mkdir()
            for name in ("fuse.h", "fuse_opt.h"):
                (include / name).write_bytes(b"/* Copyright fixture. Redistribution permitted. */\n#ifndef FIXTURE\n")
            binary = root / "cryfs"
            linked = "\t-lrefuse.2 => /usr/lib/librefuse.so.2\n\t-lboost.1 => /usr/pkg/lib/libboost.so.1\n\t-lspdlog.1 => /usr/pkg/lib/libspdlog.so.1\n\t-lfmt.12 => /usr/pkg/lib/libfmt.so.12\n"
            owners = {"/usr/pkg/lib/libboost.so.1": "boost-libs-1.92.0", "/usr/pkg/lib/libspdlog.so.1": "spdlog-1.17.0", "/usr/pkg/lib/libfmt.so.12": "fmtlib-12.2.0", "/usr/pkg/include/boost/version.hpp": "boost-headers-1.92.0", "/usr/pkg/include/range/v3/all.hpp": "range-v3-0.10.0nb1"}
            def capture(argv, cwd):
                if argv[0] == "ldd":
                    return linked
                if argv[1] == "-Fe":
                    return owners[argv[2]]
                owner = argv[2]
                license_name = "mit" if owner.startswith(("spdlog-", "fmtlib-")) else "boost-license"
                return f"Information for {owner}:\nLICENSE={license_name}\nPKGPATH=devel/fixture\n"
            def notice(name, cache, destination):
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(name)
                return {"source": "fixture", "sha256": "0" * 64, "notice": "LICENSE"}
            with patch.object(BUILD, "capture", side_effect=capture), patch.object(BUILD, "pinned_netbsd_notice", side_effect=notice):
                observed, packages = BUILD.netbsd_runtime_packages(binary, root, root / "notices", include)
                self.assertEqual(linked, observed)
                self.assertEqual(5, len(packages))
                self.assertEqual(3, sum(bool(p["libraries"]) for p in packages))
                self.assertEqual(2, sum(bool(p["headers"]) for p in packages))
                self.assertTrue((root / "notices/NetBSD-base-ReFUSE/fuse.h.notice").is_file())
                linked += "\t-lfuse.2 => /usr/pkg/lib/libfuse.so.2\n"
                with self.assertRaisesRegex(RuntimeError, "base ReFUSE"):
                    BUILD.netbsd_runtime_packages(binary, root, root / "rejected", include)

    def test_native_netbsd_patch_does_not_change_other_os_input_sets(self):
        self.assertEqual(BUILD.selected_patches("cryfs", "NetBSD"), ["native/folder/cryfs/fsyncdir.patch", "native/folder/cryfs/netbsd-platform.patch"])
        self.assertEqual(BUILD.selected_patches("cryfs", "FreeBSD"), ["native/folder/cryfs/fsyncdir.patch"])

if __name__ == "__main__":
    unittest.main()
