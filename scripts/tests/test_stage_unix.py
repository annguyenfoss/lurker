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
        self.root = Path(self.temporary.name).resolve()

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

    def test_raid1c_staging_rejects_other_unix_targets_before_output(self):
        source = mock.Mock()
        args = argparse.Namespace(source_root=self.root, source_commit="a" * 40, source_manifest=None,
                                  select=["raid1c"], backend=[], output=self.root / "package")
        for system in ("Linux", "FreeBSD", "DragonFly", "NetBSD"):
            with self.subTest(system=system):
                with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value=system):
                    with self.assertRaisesRegex(ValueError, "outside approved scope: raid1c/" + system):
                        stager.stage(args)
                self.assertFalse(args.output.exists())

    def test_unresolved_library_and_developer_rpath_fail_closed(self):
        binary = self.root / "binary"
        binary.write_bytes(b"not executed")
        for info, message in (({"needed": ["liblurker-impossible.so.999"], "search": [], "interpreters": []}, "unresolved"),
                              ({"needed": [], "search": ["/home/developer/lib"], "interpreters": []}, "untrusted")):
            with mock.patch.object(stager, "elf_info", return_value=info):
                with self.assertRaisesRegex(ValueError, message):
                    stager.audit_dependencies(self.root)

    def test_symbol_versions_are_separated_by_provider_and_definition(self):
        text = """Version symbols section '.gnu.version' contains 3 entries:
  000: 2 (GLIBC_999.0)
Version definition section '.gnu.version_d' contains 2 entries:
  000000: Rev: 1 Flags: BASE Index: 1 Cnt: 1 Name: libexample.so.1
  0x001c: Rev: 1 Flags: none Index: 2 Cnt: 1 Name: EXAMPLE_1.0
Version needs section '.gnu.version_r' contains 2 entries:
  000000: Version: 1 File: libc.so.6 Cnt: 1
  0x0010: Name: GLIBC_2.38 Flags: none Version: 3
  0x0020: Version: 1 File: libstdc++.so.6 Cnt: 2
  0x0030: Name: GLIBCXX_3.4.26 Flags: none Version: 4
  0x0040: Name: CXXABI_1.3.9 Flags: none Version: 5
"""
        needs, definitions = stager.parse_elf_versions(text)
        self.assertEqual({"libc.so.6": ["GLIBC_2.38"], "libstdc++.so.6": ["CXXABI_1.3.9", "GLIBCXX_3.4.26"]}, needs)
        self.assertEqual(["EXAMPLE_1.0", "libexample.so.1"], definitions)

    def test_matching_soname_with_missing_symbol_version_is_rejected(self):
        binary = self.root / "binary"
        library = self.root / "libexample.so.1"
        binary.write_bytes(b"not executed")
        library.write_bytes(b"not executed")
        def inspect(path, _):
            return {"needed": [library.name] if path == binary else [], "search": ["$ORIGIN"],
                    "interpreters": [], "version_needs": {library.name: ["EXAMPLE_2.0"]} if path == binary else {},
                    "version_definitions": ["EXAMPLE_1.0"] if path == library else []}
        with mock.patch.object(stager, "elf_info", side_effect=inspect):
            with self.assertRaisesRegex(ValueError, "lacks required symbol versions: EXAMPLE_2.0"):
                stager.audit_dependencies(self.root)
        def compatible(path, tool):
            info = inspect(path, tool)
            if path == library:
                info["version_definitions"].append("EXAMPLE_2.0")
            return info
        with mock.patch.object(stager, "elf_info", side_effect=compatible):
            records = stager.audit_dependencies(self.root)
        record = next(item for item in records if item["file"] == "binary")
        self.assertEqual(["EXAMPLE_2.0"], record["dependencies"][0]["required_versions"])

    def test_glibc_cannot_be_hidden_in_packaged_library_by_renaming(self):
        path = self.root / "innocent-name.so"
        path.write_bytes(b"not executed")
        info = {"needed": [], "search": [], "interpreters": [], "soname": ["libc.so.6"]}
        with mock.patch.object(stager, "elf_info", return_value=info), mock.patch.object(stager.platform, "system", return_value="Linux"):
            with self.assertRaisesRegex(ValueError, "glibc/system loader cannot be bundled"):
                stager.audit_dependencies(self.root)

    def test_real_elf_dependency_audit_executes_metadata_tools_only(self):
        binary = self.root / "never-execute-payload"
        # OpenBSD /bin/sh is a static PIE; the installed SSH client exercises dependencies.
        shutil.copyfile(shutil.which("ssh") or "/bin/sh", binary)
        original_run = stager.subprocess.run
        def metadata_only(argv, **kwargs):
            self.assertIn(Path(argv[0]).name, {"readelf", "llvm-readelf", "ldconfig"})
            return original_run(argv, **kwargs)
        with mock.patch.object(stager.subprocess, "run", side_effect=metadata_only):
            records = stager.audit_dependencies(self.root)
        self.assertTrue(records)
        self.assertTrue(any(item["bundled"] for item in records))
        self.assertTrue(any(not dependency["bundled"] for item in records for dependency in item["dependencies"]))

    def test_fuse_dependencies_distinguish_cryfs2_and_cryptomator3(self):
        self.assertEqual(["fusermount"], stager.TOOLS["Linux"]["cryfs"])
        self.assertEqual(["fusermount3"], stager.TOOLS["Linux"]["cryptomator"])
        self.assertEqual([["fusermount3", "fusermount"]], stager.TOOLS["Linux"]["gocryptfs"])
        self.assertEqual(["mount_fusefs"], stager.TOOLS["FreeBSD"]["gocryptfs"])

    def test_folder_targets_use_go_or_native_target_not_build_host(self):
        with mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.platform, "machine", return_value="x86_64"):
            stager.verify_platform({"target": {"GOOS": "linux", "GOARCH": "amd64"}, "build_platform": "Other-build-host"})
            stager.verify_platform({"target": {"os": "Linux", "machine": "x86_64"}, "build_platform": "Other-build-host"})
            with self.assertRaisesRegex(ValueError, "OS/architecture"):
                stager.verify_platform({"target": {"GOOS": "freebsd", "GOARCH": "amd64"}, "build_platform": "Linux"})
        with mock.patch.object(stager.platform, "system", return_value="FreeBSD"), mock.patch.object(stager.platform, "machine", return_value="amd64"):
            stager.verify_platform({"target": {"GOOS": "freebsd", "GOARCH": "amd64"}, "build_platform": "Linux"})

    def test_folder_durability_patch_provenance_rejects_stale_or_missing_patch(self):
        import hashlib
        name = "native/folder/cryfs/fsyncdir.patch"
        source = mock.Mock()
        source.constant.return_value = {"cryfs": [name]}
        source.read.return_value = b"final durability patch"
        correct = {"patches": {name: hashlib.sha256(source.read.return_value).hexdigest()}}
        stager.verify_folder_patches("cryfs", correct, source)
        for stale in ({}, {"patches": {}}, {"patches": {name: "old-patch-sha256"}}):
            with self.assertRaisesRegex(ValueError, "patch provenance"):
                stager.verify_folder_patches("cryfs", stale, source)
        stager.verify_folder_patches("gocryptfs", {}, source)
        stager.verify_folder_patches("gocryptfs", {"patches": {}}, source)
        with self.assertRaisesRegex(ValueError, "patch provenance"):
            stager.verify_folder_patches("gocryptfs", correct, source)

    def test_bsd_patch_identity_binds_both_patches_to_exact_source_bytes(self):
        import hashlib
        patches = {"native/veracrypt/utf8-console.patch": b"shared utf8 patch",
                   "native/bsd-veracrypt/auxiliary-path.patch": b"bsd auxiliary patch"}
        constants = {"COMMON_PATCH_SHA256": hashlib.sha256(patches["native/veracrypt/utf8-console.patch"]).hexdigest(),
                     "PATCH_SHA256": hashlib.sha256(patches["native/bsd-veracrypt/auxiliary-path.patch"]).hexdigest()}
        source = mock.Mock()
        source.read.side_effect = lambda name: patches[name]
        source.constant.side_effect = lambda path, name: constants[name]
        manifest = {"patches": [{"path": name, "sha256": hashlib.sha256(data).hexdigest()} for name, data in patches.items()]}
        stager.verify_bsd_patches(manifest, source)
        for absent in ([], manifest["patches"][:1], manifest["patches"][1:]):
            with self.assertRaisesRegex(ValueError, "artifact patch provenance"):
                stager.verify_bsd_patches({"patches": absent}, source)
        for name in list(patches):
            original = patches[name]
            patches[name] = b"changed patch"
            with self.assertRaisesRegex(ValueError, "reviewed digest"):
                stager.verify_bsd_patches(manifest, source)
            patches[name] = original

    def test_private_jvm_preload_requires_qualified_loader_anchors(self):
        runtime = self.root / "libexec/lurker/backends/cryptomator/runtime"
        for name in ("bin/java", "lib/libjli.so", "lib/server/libjvm.so", "lib/libjava.so"):
            path = runtime / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"\x7fELFfixture")
        manifest = {"backend": "cryptomator", "protocol": 1, "os": stager.platform.system(),
                    "architecture": stager.platform.machine()}
        (runtime.parent / "manifest.json").write_text(json.dumps(manifest))
        (runtime / "lib/jvm.cfg").write_text("-server KNOWN\n-client IGNORE\n")
        with mock.patch.object(stager, "elf_info", return_value={"soname": ["libjvm.so"]}):
            self.assertEqual(runtime / "lib/server/libjvm.so", stager.private_jre_preload(runtime / "lib/libjava.so", "libjvm.so", self.root, "readelf"))
            self.assertIsNone(stager.private_jre_preload(self.root / "other/libjava.so", "libjvm.so", self.root, "readelf"))
            self.assertIsNone(stager.private_jre_preload(runtime / "lib/libjava.so", "arbitrary.so", self.root, "readelf"))
            (runtime / "lib/jvm.cfg").unlink()
            with self.assertRaisesRegex(ValueError, "server JVM loader path"):
                stager.private_jre_preload(runtime / "lib/libjava.so", "libjvm.so", self.root, "readelf")

    def _linux_closure(self):
        (self.root / "lib").mkdir()
        (self.root / "licenses").mkdir()
        (self.root / "lib/libexample.so.1").write_bytes(b"private library")
        (self.root / "licenses/example.txt").write_text("upstream license")
        (self.root / "lurker-veracrypt-create").write_bytes(b"creator")
        manifest = {"schema": 1, "libraries": [{"name": "libexample.so.1", "soname": "libexample.so.1",
                    "sha256": stager.digest(self.root / "lib/libexample.so.1"), "source_path": "/usr/lib/libexample.so.1",
                    "license_files": ["licenses/example.txt"]}], "files": {
                    name: {"sha256": stager.digest(self.root / name)} for name in ("lib/libexample.so.1", "licenses/example.txt")}}
        self._save_closure(manifest)
        def elf(path, _):
            return {"needed": ["libc.so.6"] if path.parent.name == "lib" else ["libexample.so.1"],
                    "search": ["$ORIGIN"] if path.parent.name == "lib" else ["$ORIGIN/lib"],
                    "soname": [path.name] if path.parent.name == "lib" else [], "interpreters": []}
        return manifest, elf

    def _save_closure(self, manifest):
        (self.root / "private-libraries.json").write_text(json.dumps(manifest))

    def test_linux_private_closure_binds_bytes_sonames_notices_and_origin(self):
        manifest, elf = self._linux_closure()
        with mock.patch.object(stager, "elf_info", side_effect=elf):
            self.assertEqual(manifest, stager.verify_linux_private_libraries(self.root, "veracrypt-create"))
            for field, value, message in (("license_files", [], "license"), ("source_path", "", "provenance"),
                                           ("soname", "libdifferent.so.1", "SONAME"), ("sha256", "stale", "checksum")):
                original = manifest["libraries"][0][field]
                manifest["libraries"][0][field] = value
                self._save_closure(manifest)
                with self.assertRaisesRegex(ValueError, message):
                    stager.verify_linux_private_libraries(self.root, "veracrypt-create")
                manifest["libraries"][0][field] = original
            self._save_closure(manifest)
            (self.root / "lib/libunrecorded.so").write_bytes(b"unrecorded")
            with self.assertRaisesRegex(ValueError, "unmanifested"):
                stager.verify_linux_private_libraries(self.root, "veracrypt-create")

    def test_linux_private_closure_refuses_host_runtime_and_ambient_fallback(self):
        manifest, elf = self._linux_closure()
        for name in ("libc.so.6", "ld-linux-x86-64.so.2", "libpthread.so.0", "libresolv.so.2"):
            with self.subTest(name=name):
                manifest["libraries"][0]["name"] = name
                self._save_closure(manifest)
                with self.assertRaisesRegex(ValueError, "forbidden"):
                    stager.verify_linux_private_libraries(self.root, "veracrypt-create")
        manifest["libraries"][0]["name"] = "libexample.so.1"
        self._save_closure(manifest)
        def missing(path, tool):
            info = elf(path, tool)
            if path.name == "lurker-veracrypt-create":
                info["needed"].append("libfuse3.so.4")
            return info
        with mock.patch.object(stager, "elf_info", side_effect=missing):
            with self.assertRaisesRegex(ValueError, "missing dependency libfuse3.so.4"):
                stager.verify_linux_private_libraries(self.root, "veracrypt-create")
        def wrong_origin(path, tool):
            info = elf(path, tool)
            info["search"] = ["/usr/lib"]
            return info
        with mock.patch.object(stager, "elf_info", side_effect=wrong_origin):
            with self.assertRaisesRegex(ValueError, r"exact \$ORIGIN"):
                stager.verify_linux_private_libraries(self.root, "veracrypt-create")

    def test_creator_only_staging_keeps_private_libraries_and_provenance(self):
        args, source = self._stage_fixture()
        directory = self.root / "creator"
        (directory / "lib").mkdir(parents=True)
        (directory / "licenses").mkdir()
        for name in ("lurker-veracrypt-create", "creator-source.json", "private-libraries.json", "lib/libexample.so.1", "licenses/example.txt"):
            (directory / name).write_text(name)
        args.select = ["veracrypt"]
        args.backend = ["veracrypt-create=" + str(directory)]
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager, "validate_backend", return_value=directory), mock.patch.object(stager, "audit_dependencies", return_value=[]):
            self.assertEqual(0, stager.stage(args))
        self.assertEqual("lib/libexample.so.1", (args.output / "libexec/lurker/backends/lib/libexample.so.1").read_text())
        self.assertEqual("private-libraries.json", (args.output / "share/lurker/provenance/veracrypt-create-libraries.json").read_text())

    def test_cryptomator_helper_runtime_sources_match_selected_commit(self):
        names = ("src/main/java/org/lurker/cryptomator/Main.java", "src/main/java/module-info.java", "pom.xml")
        for name in names:
            path = self.root / "source" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(name.encode())
        source = mock.Mock()
        source.read.side_effect = lambda name: name.removeprefix("native/folder/cryptomator/").encode()
        stager.verify_cryptomator_sources(self.root, source)
        for name in names:
            path = self.root / "source" / name
            original = path.read_bytes()
            path.write_bytes(b"older helper")
            with self.assertRaisesRegex(ValueError, "helper source differs"):
                stager.verify_cryptomator_sources(self.root, source)
            path.write_bytes(original)

    def _stage_fixture(self):
        binaries = self.root / "binaries"
        binaries.mkdir()
        for name in ("lurker", "lurker-helper"):
            (binaries / name).write_bytes(b"\x7fELFfixture-only-never-executed")
            (binaries / name).chmod(0o755)
        provenance = {"schema": 1, "backend": "binaries", "source_commit": "a" * 40, "os": "Linux",
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

    def _raid1c_stage_fixture(self):
        args, source = self._stage_fixture()
        args.select = ["raid1c"]
        provenance = json.loads(args.binary_manifest.read_text())
        provenance["os"] = "OpenBSD"
        args.binary_manifest.write_text(json.dumps(provenance))
        return args, source

    def test_raid1c_stages_base_tools_without_private_runtime_or_qualification_claim(self):
        args, source = self._raid1c_stage_fixture()
        tools = ["bioctl", "vnconfig", "disklabel", "newfs", "mount", "umount"]
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="OpenBSD"), mock.patch.object(stager.shutil, "which", side_effect=lambda name, **kwargs: "/sbin/" + name) as lookup, mock.patch.object(stager, "audit_dependencies", return_value=[]), mock.patch.object(stager, "validate_backend") as private_runtime:
            self.assertEqual(0, stager.stage(args))
        self.assertEqual(tools, [call.args[0] for call in lookup.call_args_list])
        private_runtime.assert_not_called()
        manifest = json.loads((args.output / "share/lurker/manifest.json").read_text())
        self.assertEqual(["raid1c"], manifest["selected_backends"])
        self.assertEqual({"raid1c": tools}, manifest["host_tool_requirements"])
        self.assertEqual({tool: "/sbin/" + tool for tool in tools}, manifest["host_tools"])
        self.assertEqual([], manifest["bundled_artifacts"])
        self.assertFalse(manifest["self_contained"])
        self.assertEqual("not performed by staging", manifest["native_operation_qualification"])
        self.assertEqual([], list((args.output / "libexec/lurker/backends").iterdir()))

    def test_raid1c_missing_native_tool_blocks_publication(self):
        args, source = self._raid1c_stage_fixture()
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="OpenBSD"), mock.patch.object(stager.shutil, "which", side_effect=lambda name, **kwargs: None if name == "bioctl" else "/sbin/" + name):
            with self.assertRaisesRegex(ValueError, "host tool missing: bioctl"):
                stager.stage(args)
        self.assertFalse(args.output.exists())
        self.assertFalse(list(self.root.glob(".package-*")))

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
