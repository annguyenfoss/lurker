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
import zipfile
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

    def test_fuse_implementation_libraries_remain_host_prerequisites(self):
        for name in ("libfuse.so.2", "libfuse3.so.3.17", "librefuse.so.2", "libpuffs.so.2"):
            directory = self.root / name
            directory.mkdir()
            (directory / name).write_bytes(b"fixture never executed")
            with self.subTest(name=name), mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager.subprocess, "run", return_value=mock.Mock(stdout="")):
                with self.assertRaisesRegex(ValueError, "host prerequisite"):
                    stager.audit_dependencies(directory)
        disguised = self.root / "renamed"
        disguised.mkdir()
        (disguised / "innocent.so").write_bytes(b"fixture never executed")
        info = {"soname": ["libfuse.so.2"], "needed": [], "search": [], "interpreters": []}
        with mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager.subprocess, "run", return_value=mock.Mock(stdout="")), mock.patch.object(stager, "elf_info", return_value=info):
            with self.assertRaisesRegex(ValueError, "under another filename"):
                stager.audit_dependencies(disguised)
            info["soname"] = ["libindependentcrypto.so.1"]
            self.assertEqual(1, len(stager.audit_dependencies(disguised)))
        embedded = self.root / "embedded"
        embedded.mkdir()
        with zipfile.ZipFile(embedded / "runtime.jar", "w") as jar:
            jar.writestr("native/libfuse3.so.3", b"fixture never executed")
        with mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager.subprocess, "run", return_value=mock.Mock(stdout="")), mock.patch.object(stager, "elf_info", return_value=None):
            with self.assertRaisesRegex(ValueError, "inside a JAR"):
                stager.audit_dependencies(embedded)

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
        self.assertIn("FreeBSD", stager.ALLOW["cryfs"])
        self.assertEqual(["mount_fusefs", "umount"], stager.TOOLS["FreeBSD"]["cryfs"])

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
        source.constant.side_effect = lambda _, constant: {"cryfs": [name]} if constant == "PATCHES" else {}
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

    def test_platform_publication_patches_do_not_change_freebsd_inputs(self):
        import ast
        def constant(name, variable):
            for node in ast.parse((stager.ROOT / name).read_bytes()).body:
                if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == variable for t in node.targets):
                    return ast.literal_eval(node.value)
            raise AssertionError("Source constant missing")
        source = mock.Mock()
        source.constant.side_effect = constant
        source.read.side_effect = lambda name: (stager.ROOT / name).read_bytes()
        common = "native/folder/cryfs/fsyncdir.patch"
        local = "native/folder/cryfs/linux-localstate.patch"
        publication = "native/folder/cryfs/linux-exclusive-publication.patch"
        hashes = {name: stager.digest(stager.ROOT / name) for name in (common, local, publication)}
        with mock.patch.object(stager.platform, "system", return_value="Linux"):
            stager.verify_folder_patches("cryfs", {"patches": hashes}, source)
            with self.assertRaisesRegex(ValueError, "patch provenance"):
                stager.verify_folder_patches("cryfs", {"patches": {name: hashes[name] for name in (common, local)}}, source)
        mac = [common, "native/macos/cryfs-fuse-t.patch", "native/folder/cryfs/macos-exclusive-publication.patch"]
        mac_hashes = {name: stager.digest(stager.ROOT / name) for name in mac}
        with mock.patch.object(stager.platform, "system", return_value="Darwin"):
            stager.verify_folder_patches("cryfs", {"patches": mac_hashes}, source)
            with self.assertRaisesRegex(ValueError, "patch provenance"):
                stager.verify_folder_patches("cryfs", {"patches": {name: mac_hashes[name] for name in mac[:-1]}}, source)
        with mock.patch.object(stager.platform, "system", return_value="FreeBSD"):
            stager.verify_folder_patches("cryfs", {"patches": {common: hashes[common]}}, source)

    def test_freebsd_gocryptfs_requires_its_platform_patch(self):
        name = "native/folder/gocryptfs/freebsd-futimes.patch"
        patch = stager.ROOT / name
        source = mock.Mock()
        source.constant.side_effect = lambda _, key: {"FreeBSD": {"gocryptfs": [name]}} if key == "PLATFORM_PATCHES" else {}
        source.read.return_value = patch.read_bytes()
        patched = {"patches": {name: stager.digest(patch)}}
        with mock.patch.object(stager.platform, "system", return_value="FreeBSD"):
            stager.verify_folder_patches("gocryptfs", patched, source)
            with self.assertRaisesRegex(ValueError, "patch provenance"):
                stager.verify_folder_patches("gocryptfs", {}, source)
        with mock.patch.object(stager.platform, "system", return_value="Linux"):
            stager.verify_folder_patches("gocryptfs", {}, source)
            with self.assertRaisesRegex(ValueError, "patch provenance"):
                stager.verify_folder_patches("gocryptfs", patched, source)

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

    def test_bsd_patches_require_the_platform_specific_patches(self):
        source = mock.Mock()
        source.read.side_effect = lambda name: name.encode()
        entries = [
            ("native/veracrypt/utf8-console.patch", "COMMON_PATCH_SHA256"),
            ("native/bsd-veracrypt/auxiliary-path.patch", "PATCH_SHA256"),
            ("native/bsd-veracrypt/openbsd-vnd-io.patch", "OPENBSD_VND_PATCH_SHA256"),
            ("native/bsd-veracrypt/openbsd-ffs-shutdown.patch", "OPENBSD_FFS_SHUTDOWN_PATCH_SHA256"),
            ("native/bsd-veracrypt/openbsd-device-discovery.patch", "OPENBSD_DEVICE_DISCOVERY_PATCH_SHA256"),
            ("native/bsd-veracrypt/freebsd-md-sync.patch", "FREEBSD_MD_PATCH_SHA256"),
        ]
        hashes = {constant: stager.hashlib.sha256(name.encode()).hexdigest() for name, constant in entries}
        source.constant.side_effect = lambda _path, name: hashes[name]
        provenance = [{"path": name, "sha256": hashes[constant]} for name, constant in entries]
        common = provenance[:2]
        openbsd = provenance[2:5]
        freebsd = provenance[5:]
        for system, local, other in (("OpenBSD", openbsd, freebsd), ("FreeBSD", freebsd, openbsd)):
            with self.subTest(platform=system):
                stager.verify_bsd_patches({"platform": system, "patches": common + local}, source)
                for invalid in (common + local[:-1], common + other, provenance):
                    with self.assertRaisesRegex(ValueError, "artifact patch provenance"):
                        stager.verify_bsd_patches({"platform": system, "patches": invalid}, source)

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

    def _static_wx(self):
        source = mock.Mock()
        constants = {"VERSION": "3.2.8.1", "URL": "https://example.test/pinned-wx.tar.bz2", "SHA256": "a" * 64,
                     "FLAGS": ("--disable-shared", "--disable-secretstore"), "LICENSES": ("docs/licence.txt", "3rdparty/pcre/LICENCE"),
                     "ENV_FLAGS": ("CFLAGS", "CXXFLAGS", "CPPFLAGS", "LDFLAGS")}
        source.constant.side_effect = lambda path, key: constants[key]
        notices = ["licenses/wxwidgets/" + name for name in constants["LICENSES"]]
        for name in notices:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(name)
        receipt = {"schema": 1, "inputs": {"version": constants["VERSION"], "url": constants["URL"],
                    "archive_sha256": constants["SHA256"], "flags": ["--prefix=/private/build", *constants["FLAGS"]],
                    "source_tree_sha256": "b" * 64, "source_dir": "/private/source", "environment": {key: "" for key in constants["ENV_FLAGS"]},
                    "compilers": {key: {"path": "/usr/bin/compiler", "version": "native compiler", "sha256": "c" * 64} for key in ("CC", "CXX")}},
                   "static_libraries": {name: "d" * 64 for name in ("lib/libwx_baseu-3.2.a", "lib/libwxregexu-3.2.a")}, "license_files": notices}
        path = "licenses/wxwidgets/provenance.json"
        manifest = {"static_libraries": [{"name": "wxWidgets", "provenance": path, "license_files": notices}],
                    "files": {name: {"sha256": stager.digest(self.root / name)} for name in notices}}
        self._save_wx(receipt, manifest)
        return source, receipt, manifest

    def _save_wx(self, receipt, manifest):
        path = self.root / "licenses/wxwidgets/provenance.json"
        path.write_text(json.dumps(receipt))
        manifest["files"][str(path.relative_to(self.root))] = {"sha256": stager.digest(path)}

    def test_static_wx_is_bound_to_source_configuration_and_notices(self):
        source, receipt, manifest = self._static_wx()
        stager.verify_linux_static_wx(self.root, manifest, source)
        path = self.root / "licenses/wxwidgets/provenance.json"
        original = path.read_text()
        path.write_text(original + " ")
        with self.assertRaisesRegex(ValueError, "checksum"):
            stager.verify_linux_static_wx(self.root, manifest, source)
        path.write_text(original)
        for key, value, message in (("version", "stale", "source pin"), ("archive_sha256", "stale", "source pin"),
                                     ("flags", ["--prefix=/private", "--enable-secretstore"], "configuration"),
                                     ("source_tree_sha256", "", "source-tree"), ("compilers", {}, "compiler identity"),
                                     ("environment", {}, "compiler flag")):
            with self.subTest(key=key):
                before = receipt["inputs"][key]
                receipt["inputs"][key] = value
                self._save_wx(receipt, manifest)
                with self.assertRaisesRegex(ValueError, message):
                    stager.verify_linux_static_wx(self.root, manifest, source)
                receipt["inputs"][key] = before
        self._save_wx(receipt, manifest)
        receipt["license_files"] = receipt["license_files"][:1]
        self._save_wx(receipt, manifest)
        with self.assertRaisesRegex(ValueError, "license list"):
            stager.verify_linux_static_wx(self.root, manifest, source)

    def test_missing_wx_archives_or_dynamic_substitution_are_refused(self):
        source, receipt, manifest = self._static_wx()
        receipt["static_libraries"].pop("lib/libwxregexu-3.2.a")
        self._save_wx(receipt, manifest)
        with self.assertRaisesRegex(ValueError, "archive identities"):
            stager.verify_linux_static_wx(self.root, manifest, source)
        manifest["libraries"] = [{"soname": "libwx_baseu-3.2.so.0"}]
        with self.assertRaisesRegex(ValueError, "linked statically"):
            stager.verify_linux_static_wx(self.root, manifest, source)
        manifest["static_libraries"] = []
        with self.assertRaisesRegex(ValueError, "pinned private static"):
            stager.verify_linux_static_wx(self.root, manifest, source)

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
        def host_glibc(path, tool):
            info = elf(path, tool)
            info["needed"].extend(["libresolv.so.2", "libutil.so.1", "libanl.so.1"])
            return info
        with mock.patch.object(stager, "elf_info", side_effect=host_glibc):
            stager.verify_linux_private_libraries(self.root, "veracrypt-create")
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

    def test_bsd_veracrypt_requires_complete_wxwidgets_notices(self):
        engine = self.root / "lurker-veracrypt-bsd"
        engine.write_bytes(b"\x7fELFfixture")
        engine.chmod(0o755)
        (self.root / "licenses").mkdir()
        exception = self.root / "licenses/wxWidgets.txt"
        exception.write_text("Exception notice")
        pins = {"veracrypt": {"revision": "qualified"}}
        notices = {"docs/licence.txt": "wxWidgets.txt", "docs/lgpl.txt": "wxWidgets-LGPL.txt"}
        source = mock.Mock()
        source.constant.side_effect = lambda _, name: {"PINS": pins, "WX_NOTICES": notices}[name]
        manifest = {"sources": pins, "binary": engine.name, "binary_sha256": stager.digest(engine),
                    "files": {engine.name: stager.digest(engine), "licenses/wxWidgets.txt": stager.digest(exception)}}
        receipt = self.root / "bsd-veracrypt-source.json"
        receipt.write_text(json.dumps(manifest))
        with mock.patch.object(stager, "verify_platform"), mock.patch.object(stager, "verify_bsd_patches"), mock.patch.object(stager, "verify_bsd_host_fuse"):
            with self.assertRaisesRegex(ValueError, "wxWidgets license texts are missing"):
                stager.validate_backend("veracrypt-bsd", self.root, source)
            library_license = "licenses/wxWidgets-LGPL.txt"
            (self.root / library_license).write_text("Library GPL")
            manifest["files"][library_license] = stager.digest(self.root / library_license)
            receipt.write_text(json.dumps(manifest))
            self.assertEqual(self.root, stager.validate_backend("veracrypt-bsd", self.root, source))

    def test_bsd_host_fuse_accepts_compatible_updates_and_records_current_provider(self):
        provider = self.root / "provider"
        provider.write_bytes(b"updated administrator-installed compatible library")
        host = {"api": "fuse2", "library": "libfuse.so.2", "path": "/usr/local/lib/libfuse.so.2", "sha256": "a" * 64}
        info = {"soname": [host["library"]]}
        engine = {"needed": [host["library"], "libc.so.7"]}
        with mock.patch.object(stager.platform, "system", return_value="FreeBSD"), \
                mock.patch.object(stager, "administrator_file", return_value=Path("/usr/local/lib/libfuse.so.2.9")), \
                mock.patch.object(stager.shutil, "which", return_value="/usr/bin/readelf"), \
                mock.patch.object(stager, "elf_info", side_effect=[info, engine]), \
                mock.patch.object(stager, "digest", return_value=stager.digest(provider)):
            observed = stager.verify_bsd_host_fuse(self.root, {"host_fuse": host})
        self.assertEqual(observed["sha256"], stager.digest(provider))
        self.assertEqual(observed["build_sha256"], "a" * 64)
        self.assertNotEqual(observed["sha256"], observed["build_sha256"])
        for field, invalid in (("api", "fuse3"), ("path", "/tmp/libfuse.so.2"), ("library", "libfuse.so.3"), ("sha256", "bad")):
            with mock.patch.object(stager.platform, "system", return_value="FreeBSD"):
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, "ABI provenance"):
                    stager.verify_bsd_host_fuse(self.root, {"host_fuse": {**host, field: invalid}})
        with mock.patch.object(stager.platform, "system", return_value="FreeBSD"), \
                mock.patch.object(stager, "administrator_file", return_value=Path("/usr/local/lib/libfuse.so.2")), \
                mock.patch.object(stager.shutil, "which", return_value="/usr/bin/readelf"), \
                mock.patch.object(stager, "elf_info", side_effect=[{"soname": ["libfuse.so.3"]}, engine]):
            with self.assertRaisesRegex(ValueError, "SONAME/engine ABI"):
                stager.verify_bsd_host_fuse(self.root, {"host_fuse": host})

    def test_host_prerequisite_checks_symlink_target_and_all_ancestors(self):
        provider = self.root / "libfuse.so.2.9"
        provider.write_bytes(b"host fixture")
        alias = self.root / "libfuse.so.2"
        alias.symlink_to(provider.name)
        real_lstat = Path.lstat
        def administrator_stat(path):
            original = real_lstat(path)
            values = list(original)
            values[4] = 0
            values[0] &= ~0o022
            return os.stat_result(values)
        with mock.patch.object(Path, "lstat", administrator_stat):
            self.assertEqual(stager.administrator_file(alias), provider)
        def writable_target(path):
            info = administrator_stat(path)
            values = list(info)
            if path == provider:
                values[0] |= 0o020
            return os.stat_result(values)
        with mock.patch.object(Path, "lstat", writable_target):
            with self.assertRaisesRegex(ValueError, "administrator-controlled"):
                stager.administrator_file(alias)
        def unowned_ancestor(path):
            info = administrator_stat(path)
            values = list(info)
            if path == self.root:
                values[4] = 1234
            return os.stat_result(values)
        with mock.patch.object(Path, "lstat", unowned_ancestor):
            with self.assertRaisesRegex(ValueError, "administrator-controlled"):
                stager.administrator_file(alias)

    def test_rust_notices_follow_the_build_receipt_and_verify_registry_bytes(self):
        source = mock.Mock(root=self.root / "project", commit="a" * 40)
        source.root.mkdir()
        product_manifest = source.root / "Cargo.toml"
        product_manifest.touch()
        binary = self.root / "lurker"
        binary.touch()
        registry = self.root / "cargo/registry"
        crate = "example-crate-1.0.0"
        directory = registry / "src/mirror" / crate
        directory.mkdir(parents=True)
        (directory / "Cargo.toml").write_text('[package]\nname="example-crate"\nversion="1.0.0"\nlicense-file="terms.txt"\n')
        (directory / "terms.txt").write_text("Declared license")
        (directory / "NOTICE").write_text("Exact upstream notice")
        archive = registry / "cache/mirror" / (crate + ".crate")
        archive.parent.mkdir(parents=True)
        with tarfile.open(archive, "w:gz") as stream:
            stream.add(directory, arcname=crate)
        registry_source = "registry+https://github.com/rust-lang/crates.io-index"
        source.read.return_value = (f'[[package]]\nname="example-crate"\nversion="1.0.0"\n'
                                    f'source="{registry_source}"\nchecksum="{stager.digest(archive)}"\n').encode()
        log = self.root / "rust-build.log"
        events = [{"reason": "compiler-artifact", "package_id": registry_source + "#example-crate@1.0.0",
                   "manifest_path": str(directory / "Cargo.toml")},
                  {"reason": "compiler-artifact", "package_id": "path+file:///project#0.1.0",
                   "manifest_path": str(product_manifest), "executable": str(binary)},
                  {"reason": "build-finished", "success": True}]
        log.write_text("\n".join(map(json.dumps, events)))
        destination = self.root / "notices"
        receipt = stager.record_rust_notices(log, source, binary.parent, [binary.name], destination)
        catalog = json.loads((destination / "packages.json").read_text())
        self.assertEqual(catalog["packages"][0]["notices"], ["NOTICE", "terms.txt"])
        self.assertEqual(receipt["files"][crate + "/NOTICE"]["sha256"], stager.digest(directory / "NOTICE"))
        archive.write_bytes(b"different archive")
        with self.assertRaisesRegex(ValueError, "differs from Cargo.lock"):
            stager.record_rust_notices(log, source, binary.parent, [binary.name], self.root / "new-notices")

    def test_stager_copies_and_attests_rust_notices(self):
        args, source = self._stage_fixture()
        notices = args.binary_manifest.parent / "rust-notices"
        notices.mkdir()
        (notices / "packages.json").write_text("[]")
        provenance = json.loads(args.binary_manifest.read_text())
        provenance["rust_notices"] = {"files": stager.file_manifest(notices)}
        args.binary_manifest.write_text(json.dumps(provenance))
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager, "audit_dependencies", return_value=[]):
            self.assertEqual(0, stager.stage(args))
            installed = args.output / "share/lurker/licenses/rust/packages.json"
            self.assertEqual(installed.read_text(), "[]")
            args.output = self.root / "invalid-package"
            (notices / "packages.json").write_text("modified")
            with self.assertRaisesRegex(ValueError, "notice inventory differs"):
                stager.stage(args)
            self.assertFalse(args.output.exists())

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
        self.assertEqual("private-libraries.json", (args.output / "libexec/lurker/backends/private-libraries.json").read_text())
        self.assertEqual("private-libraries.json", (args.output / "share/lurker/provenance/veracrypt-create-libraries.json").read_text())

    def test_userspace_package_keeps_creator_loader_inventory_beside_executables(self):
        args, source = self._stage_fixture()
        directory = self.root / "userspace"
        (directory / "lib").mkdir(parents=True)
        (directory / "licenses").mkdir()
        for name in (*stager.ENTRY["veracrypt-userspace"], "private-libraries.json", "lib/libexample.so.1", "licenses/example.txt"):
            (directory / name).write_text(name)
        args.select = ["veracrypt"]
        args.backend = ["veracrypt-userspace=" + str(directory)]
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager, "validate_backend", return_value=directory), mock.patch.object(stager, "audit_dependencies", return_value=[]):
            self.assertEqual(0, stager.stage(args))
        name = "libexec/lurker/backends/private-libraries.json"
        self.assertEqual("private-libraries.json", (args.output / name).read_text())
        manifest = json.loads((args.output / "share/lurker/manifest.json").read_text())
        self.assertEqual(stager.digest(args.output / name), manifest["files"][name]["sha256"])

    def test_cryptomator_helper_runtime_sources_match_selected_commit(self):
        names = ("src/main/java/org/lurker/cryptomator/Main.java", "src/main/java/module-info.java",
                 "src/main/java/org/lurker/cryptomator/Storage.java", "src/main/resources/storage.properties", "pom.xml")
        for name in names:
            path = self.root / "source" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(name.encode())
        source = mock.Mock()
        source.files.return_value = ["native/folder/cryptomator/" + name for name in names if name.startswith("src/main/")]
        source.read.side_effect = lambda name: name.removeprefix("native/folder/cryptomator/").encode()
        stager.verify_cryptomator_sources(self.root, source)
        for name in names:
            path = self.root / "source" / name
            original = path.read_bytes()
            path.write_bytes(b"older helper")
            with self.assertRaisesRegex(ValueError, "helper source differs"):
                stager.verify_cryptomator_sources(self.root, source)
            path.write_bytes(original)
        extra = self.root / "source/src/main/java/Untracked.java"
        extra.write_text("untracked helper")
        with self.assertRaisesRegex(ValueError, "source inventory differs"):
            stager.verify_cryptomator_sources(self.root, source)
        extra.unlink()
        (self.root / "source" / names[2]).unlink()
        with self.assertRaisesRegex(ValueError, "source inventory differs"):
            stager.verify_cryptomator_sources(self.root, source)

    def test_source_inventory_uses_selected_git_revision_or_export_manifest(self):
        source = object.__new__(stager.Source)
        source.root = self.root
        source.commit = "a" * 40
        source.manifest = None
        prefix = "native/folder/cryptomator/src/main/"
        names = [prefix + "java/Main.java", prefix + "resources/storage.properties"]
        with mock.patch.object(stager.subprocess, "check_output", return_value=("\0".join(names) + "\0").encode()) as command:
            self.assertEqual(names, source.files(prefix))
            self.assertEqual(source.commit, command.call_args.args[0][-3])
        source.manifest = {"files": {name: {} for name in [*names, "Cargo.toml"]}}
        self.assertEqual(names, source.files(prefix))
        with self.assertRaisesRegex(ValueError, "unsafe relative path"):
            source.files("../outside/")

    def test_cryptomator_requires_entry_sources_even_if_source_inventory_omits_them(self):
        (self.root / "source").mkdir()
        (self.root / "source/pom.xml").write_bytes(b"pom.xml")
        source = mock.Mock()
        source.files.return_value = []
        with self.assertRaisesRegex(ValueError, "source inventory differs"):
            stager.verify_cryptomator_sources(self.root, source)

    def test_cryptomator_patch_provenance_binds_source_archive_diff_and_outputs(self):
        module = "org.cryptomator.cryptofs"
        name = "org/example/Storage.java"
        old, new = "class Storage { int value = 1; }\n", "class Storage { int value = 2; }\n"
        for path in ("mods", "patches", "source/linux-patches", "source/upstream", "source/patched-modules/" + module + "/org/example"):
            (self.root / path).mkdir(parents=True, exist_ok=True)
        archive = self.root / "source/upstream/cryptofs-2.8.0-sources.jar"
        with zipfile.ZipFile(archive, "w") as output:
            output.writestr(name, old)
        patch = ("--- a/" + name + "\n+++ b/" + name + "\n@@ -1 +1 @@\n-" + old + "+" + new).encode()
        entry = {"module": module, "artifact": "org.cryptomator:cryptofs:2.8.0:jar:sources",
                 "base_jar": "cryptofs-2.8.0.jar", "patch": "crypto.patch", "jar": "crypto.jar",
                 "source_sha256": stager.digest(archive)}
        config = json.dumps({"schema": 1, "modules": [entry]}).encode()
        (self.root / "source/linux-patches/manifest.json").write_bytes(config)
        (self.root / "source/linux-patches/crypto.patch").write_bytes(patch)
        base = self.root / "mods/cryptofs-2.8.0.jar"
        base.write_bytes(b"upstream binary")
        binary = self.root / "patches/crypto.jar"
        binary.write_bytes(b"compiled patch")
        unit = self.root / "source/patched-modules" / module / name
        unit.write_text(new)
        record = {**entry, "base_sha256": stager.digest(base), "patch_sha256": stager.hashlib.sha256(patch).hexdigest(),
                  "jar_sha256": stager.digest(binary), "source_files": {name: stager.digest(unit)}}
        manifest = {"linux_patches": {"configuration_sha256": stager.hashlib.sha256(config).hexdigest(), "modules": [record], "runtime_verified": True}}
        lock = {"sha256": {base.name: stager.digest(base)}}
        prefix = "native/folder/cryptomator/linux-patches/"
        inputs = {prefix + "manifest.json": config, prefix + "crypto.patch": patch}
        source = mock.Mock()
        source.files.return_value = list(inputs)
        source.read.side_effect = inputs.__getitem__
        source.constant.return_value = {module: (entry["artifact"], entry["base_jar"])}
        stager.verify_cryptomator_patches(self.root, manifest, source, lock)
        for file, message in ((base, "base module differs"), (binary, "binary differs"), (archive, "upstream source differs")):
            original = file.read_bytes()
            file.write_bytes(b"changed")
            with self.subTest(file=file.name), self.assertRaisesRegex(ValueError, message):
                stager.verify_cryptomator_patches(self.root, manifest, source, lock)
            file.write_bytes(original)
        unit.write_text(new.replace("= 2", "= 3"))
        record["source_files"][name] = stager.digest(unit)
        with self.assertRaisesRegex(ValueError, "do not reproduce the reviewed patch"):
            stager.verify_cryptomator_patches(self.root, manifest, source, lock)

    def test_cryptomator_patch_artifact_cannot_add_patch_to_unpatched_source(self):
        source = mock.Mock()
        source.files.return_value = []
        stager.verify_cryptomator_patches(self.root, {}, source, {})
        with self.assertRaisesRegex(ValueError, "patches absent"):
            stager.verify_cryptomator_patches(self.root, {"linux_patches": {}}, source, {})

    def test_cryptomator_module_path_matches_the_actual_locked_dependency_bytes(self):
        modules = self.root / "mods"
        modules.mkdir()
        (modules / "lurker-cryptomator-1.0.0.jar").write_bytes(b"helper")
        dependency = modules / "backend.jar"
        dependency.write_bytes(b"locked dependency")
        lock = {"sha256": {dependency.name: stager.digest(dependency)}}
        stager.verify_cryptomator_dependencies(self.root, lock)
        dependency.write_bytes(b"modified")
        with self.assertRaisesRegex(ValueError, "runtime dependencies differ"):
            stager.verify_cryptomator_dependencies(self.root, lock)
        dependency.write_bytes(b"locked dependency")
        (modules / "unreviewed.module").mkdir()
        with self.assertRaisesRegex(ValueError, "unreviewed entries"):
            stager.verify_cryptomator_dependencies(self.root, lock)

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

    def test_private_umask_keeps_work_private_and_installed_runtime_readable(self):
        args, source = self._stage_fixture()
        args.select = ["cryptomator"]
        directory = self.root / "cryptomator-input"
        previous = os.umask(0o077)
        try:
            (directory / "runtime/bin").mkdir(parents=True)
            (directory / "runtime/lib/server").mkdir(parents=True)
            (directory / "licenses").mkdir()
            for name in ("lurker-cryptomator", "runtime/bin/java", "runtime/lib/server/libjvm.so",
                         "manifest.json", "licenses/notice.txt"):
                (directory / name).write_bytes(name.encode())
            for name in stager.ENTRY["cryptomator"]:
                (directory / name).chmod(0o700)
            (directory / "runtime/lib/libjvm.so").symlink_to("server/libjvm.so")
            args.backend = ["cryptomator=" + str(directory)]
            with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", return_value="/trusted/tool"), mock.patch.object(stager, "validate_backend", return_value=directory), mock.patch.object(stager, "audit_dependencies", return_value=[]):
                self.assertEqual(0, stager.stage(args))
        finally:
            os.umask(previous)
        prefix = args.output.resolve()
        self.assertEqual(0o700, stat.S_IMODE(prefix.parent.stat().st_mode))
        self.assertEqual(0o755, stat.S_IMODE(prefix.stat().st_mode))
        for path in prefix.rglob("*"):
            if path.is_symlink():
                self.assertEqual("server/libjvm.so", os.readlink(path))
            elif path.is_dir():
                self.assertEqual(0o755, stat.S_IMODE(path.stat().st_mode), str(path))
            else:
                executable = path.name in ("lurker", "lurker-helper", "lurker-cryptomator", "java")
                self.assertEqual(0o755 if executable else 0o644, stat.S_IMODE(path.stat().st_mode), str(path))
        with tarfile.open(prefix / "package.tar.gz") as archive:
            for member in archive:
                if member.isdir():
                    self.assertEqual(0o755, member.mode)
                elif member.isfile():
                    executable = Path(member.name).name in ("lurker", "lurker-helper", "lurker-cryptomator", "java")
                    self.assertEqual(0o755 if executable else 0o644, member.mode, member.name)

    def _raid1c_stage_fixture(self):
        args, source = self._stage_fixture()
        args.select = ["raid1c"]
        provenance = json.loads(args.binary_manifest.read_text())
        provenance["os"] = "OpenBSD"
        args.binary_manifest.write_text(json.dumps(provenance))
        return args, source

    def test_desktop_native_package_requires_external_authorization_executable(self):
        args, source = self._stage_fixture()
        args.desktop = True
        binary = args.binaries / "lurker-desktop"
        binary.write_bytes(b"\x7fELFfixture-never-executed")
        binary.chmod(0o755)
        provenance = json.loads(args.binary_manifest.read_text())
        provenance["files"][binary.name] = {"sha256": stager.digest(binary)}
        args.binary_manifest.write_text(json.dumps(provenance))
        def without_authorization(name, **kwargs):
            return None if name in {"sudo", "pkexec"} else "/trusted/" + name
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", side_effect=without_authorization):
            with self.assertRaisesRegex(ValueError, "pkexec or sudo plus an administrator policy"):
                stager.stage(args)
        self.assertFalse(args.output.exists())
        def sudo_present(name, **kwargs):
            return None if name == "pkexec" else "/trusted/" + name
        with mock.patch.object(stager, "Source", return_value=source), mock.patch.object(stager.platform, "system", return_value="Linux"), mock.patch.object(stager.shutil, "which", side_effect=sudo_present), mock.patch.object(stager, "audit_dependencies", return_value=[]):
            self.assertEqual(0, stager.stage(args))
        manifest = json.loads((args.output / "share/lurker/manifest.json").read_text())
        authorization = manifest["desktop_authorization"]
        self.assertEqual(["luks"], authorization["required_for"])
        self.assertEqual({"sudo": "/trusted/sudo"}, authorization["available"])
        self.assertIn("presence checked only", authorization["qualification"])
        self.assertIn("working administrator authorization", authorization["policy_requirement"])
        display = manifest["desktop_runtime"]
        self.assertIn("libxkbcommon-x11", display["display_sessions"]["X11"]["libraries"])
        self.assertIn("Wayland", display["display_sessions"])
        self.assertEqual("at least one readable system font indexed by Fontconfig", display["system_fonts"])
        self.assertIn("not performed by staging", display["qualification"])
        self.assertIn("dlopen", display["library_audit_scope"])
        self.assertEqual(["Xvfb", "xauth"], display["test_only_tools"])

    def test_bsd_desktop_declares_x11_runtime_without_claiming_display_execution(self):
        for system in ("FreeBSD", "DragonFly", "NetBSD", "OpenBSD"):
            with self.subTest(system=system):
                display = stager.desktop_display_requirements(system, True)
                self.assertEqual({"X11"}, set(display["display_sessions"]))
                self.assertIn("libxkbcommon-x11", display["display_sessions"]["X11"]["libraries"])
                self.assertIn("actual native winit window", display["qualification"])
                self.assertIsNone(stager.desktop_display_requirements(system, False))

    def test_login_user_folder_or_cli_only_does_not_require_desktop_elevation(self):
        with mock.patch.object(stager.shutil, "which") as lookup:
            self.assertIsNone(stager.desktop_authorization({"gocryptfs", "cryfs", "cryptomator"}, True))
            self.assertIsNone(stager.desktop_authorization({"luks", "geli", "cgd", "softraid", "raid1c"}, False))
        lookup.assert_not_called()
        for backend in ("luks1", "geli", "cgd", "softraid", "raid1c", "pefs", "veracrypt"):
            with self.subTest(backend=backend), mock.patch.object(stager.shutil, "which", return_value=None):
                with self.assertRaisesRegex(ValueError, "desktop native operations require"):
                    stager.desktop_authorization({backend}, True)

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
