#!/usr/bin/env python3
"""Stage verified Unix artifacts without building, installing or executing them."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
SUPPORTED = {"Linux", "FreeBSD", "DragonFly", "NetBSD", "OpenBSD"}
ALLOW = {
    "luks": {"Linux"}, "luks1": {"Linux", "DragonFly"},
    "veracrypt": {"Linux", "FreeBSD", "OpenBSD"},
    "gocryptfs": {"Linux", "FreeBSD"}, "cryfs": {"Linux"},
    "cryptomator": {"Linux"}, "geli": {"FreeBSD"}, "pefs": {"FreeBSD"},
    "cgd": {"NetBSD"}, "softraid": {"OpenBSD"}, "raid1c": {"OpenBSD"},
}
ARTIFACTS = {"veracrypt-create": "veracrypt", "veracrypt-userspace": "veracrypt",
             "veracrypt-bsd": "veracrypt", "gocryptfs": "gocryptfs",
             "cryfs": "cryfs", "cryptomator": "cryptomator"}
ENTRY = {"veracrypt-create": ["lurker-veracrypt-create"],
         "veracrypt-userspace": ["lurker-veracrypt", "lurker-fs", "lurker-veracrypt-create"],
         "veracrypt-bsd": ["lurker-veracrypt-bsd"], "gocryptfs": ["gocryptfs"],
         "cryfs": ["cryfs"], "cryptomator": ["lurker-cryptomator", "runtime/bin/java"]}
TOOLS = {
    "Linux": {"luks": ["cryptsetup", "mkfs.btrfs", "mount", "umount", "lsblk", "losetup"],
              "luks1": ["cryptsetup", "mkfs.btrfs", "mount", "umount", "lsblk", "losetup"],
              "veracrypt": ["cryptsetup", "mkfs.btrfs", "mount", "umount", "losetup"],
              "gocryptfs": [["fusermount3", "fusermount"]], "cryfs": ["fusermount"], "cryptomator": ["fusermount3"]},
    "FreeBSD": {"geli": ["geli", "mdconfig", "newfs", "mount", "umount"],
                "pefs": ["pefs", "mount", "umount"], "gocryptfs": ["mount_fusefs"]},
    "NetBSD": {"cgd": ["cgdconfig", "vnconfig", "newfs", "mount", "umount"]},
    "OpenBSD": {"softraid": ["bioctl", "vnconfig", "disklabel", "newfs", "mount", "umount"],
                "raid1c": ["bioctl", "vnconfig", "disklabel", "newfs", "mount", "umount"]},
    "DragonFly": {"luks1": ["cryptsetup", "vnconfig", "newfs", "mount", "umount"]},
}


def fail(message):
    raise ValueError(message)


def digest(path):
    hasher = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def relative(value):
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        fail("unsafe relative path: " + str(value))
    return path


def safe_root(path):
    path = Path(path).absolute()
    if path.is_symlink() or not path.is_dir():
        fail("artifact root must be a real directory: " + str(path))
    return path.resolve(strict=True)


def entries(root):
    root = safe_root(root)
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            mode = path.lstat().st_mode
            if stat.S_ISLNK(mode):
                try:
                    resolved = path.resolve(strict=True)
                except (OSError, RuntimeError) as error:
                    fail("broken/looping source symlink: " + str(path) + ": " + str(error))
                if not resolved.is_relative_to(root):
                    fail("external source symlink: " + str(path))
                yield path, "symlink"
            elif stat.S_ISREG(mode):
                if mode & (stat.S_ISUID | stat.S_ISGID):
                    fail("setuid/setgid input is forbidden: " + str(path))
                yield path, "file"
            elif stat.S_ISDIR(mode):
                continue
            else:
                fail("special file input is forbidden: " + str(path))


def file_manifest(root, exclude=()):
    return {str(path.relative_to(root)): ({"symlink": os.path.relpath(path.resolve(), path.parent)}
            if kind == "symlink" else {"sha256": digest(path), "executable": bool(path.stat().st_mode & 0o111)})
            for path, kind in entries(root) if str(path.relative_to(root)) not in exclude}


def copy_tree(source, destination):
    source = safe_root(source)
    # Validate the entire input before copying; never follow external links.
    listing = list(entries(source))
    destination.mkdir(parents=True, exist_ok=True)
    for path, kind in listing:
        target = destination / path.relative_to(source)
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() or target.is_symlink():
            fail("colliding payload path: " + str(target))
        if kind == "symlink":
            target.symlink_to(os.path.relpath(path.resolve(), path.parent))
        else:
            shutil.copyfile(path, target, follow_symlinks=False)
            target.chmod(0o755 if path.stat().st_mode & 0o111 else 0o644)


def json_file(path):
    if Path(path).is_symlink():
        fail("provenance file must not be a symlink: " + str(path))
    return json.loads(Path(path).read_text())


def architecture(value):
    return {"amd64": "x86_64", "arm64": "aarch64"}.get(value, value)


class Source:
    def __init__(self, root, commit, manifest=None):
        self.root = safe_root(root)
        if not re.fullmatch(r"[0-9a-f]{40}", commit):
            fail("source commit must be a complete 40-character Git SHA")
        self.commit = commit
        self.manifest = json_file(manifest) if manifest else None
        if self.manifest:
            if self.manifest.get("commit") != commit:
                fail("source manifest commit differs")
        else:
            actual = subprocess.check_output(["git", "-C", str(self.root), "rev-parse", commit + "^{commit}"], text=True).strip()
            if actual != commit:
                fail("source Git commit differs")

    def read(self, name):
        relative(name)
        if self.manifest:
            path = self.root / name
            expected = self.manifest["files"].get(name, {}).get("sha256")
            if path.is_symlink() or not expected or digest(path) != expected:
                fail("source manifest mismatch: " + name)
            return path.read_bytes()
        return subprocess.check_output(["git", "-C", str(self.root), "show", self.commit + ":" + name])

    def constant(self, name, variable):
        tree = ast.parse(self.read(name))
        for node in tree.body:
            if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == variable for t in node.targets):
                return ast.literal_eval(node.value)
        fail("source constant missing: " + variable)

    def verify_clean_build_source(self):
        if self.manifest:
            for name, entry in self.manifest["files"].items():
                if "sha256" in entry:
                    self.read(name)
        else:
            status = subprocess.check_output(["git", "-C", str(self.root), "status", "--porcelain", "--untracked-files=all"], text=True)
            current = subprocess.check_output(["git", "-C", str(self.root), "rev-parse", "HEAD"], text=True).strip()
            if status or current != self.commit:
                fail("record build provenance only from a clean checkout at the exact source commit")


def verify_files(root, expected):
    for name, value in expected.items():
        path = root / relative(name)
        wanted = value.get("sha256") if isinstance(value, dict) else value
        if isinstance(value, dict) and "symlink" in value:
            if not path.is_symlink() or os.path.relpath(path.resolve(), path.parent) != value["symlink"]:
                fail("artifact symlink differs: " + name)
        elif not path.is_file() or digest(path) != wanted:
            fail("artifact checksum differs: " + name)


def verify_platform(manifest):
    target = manifest.get("target")
    if target is not None:
        if not isinstance(target, dict):
            fail("native artifact target metadata is invalid")
        if "GOOS" in target:
            system = {"linux": "Linux", "freebsd": "FreeBSD", "darwin": "Darwin"}.get(target["GOOS"])
            machine = target.get("GOARCH")
        else:
            system, machine = target.get("os"), target.get("machine")
    else:
        system = manifest.get("os")
        if system is None and manifest.get("platform"):
            system = manifest["platform"].split("-")[0]
        machine = manifest.get("architecture", manifest.get("machine"))
    if system != platform.system() or architecture(machine) != architecture(platform.machine()):
        fail("native artifact OS/architecture differs from this packaging host")


def magic(path):
    with Path(path).open("rb") as stream:
        return stream.read(4)


def require_executable(path):
    if path.is_symlink() or not path.is_file() or not path.stat().st_mode & 0o111:
        fail("required executable missing or unsafe: " + str(path))


def verify_bsd_patches(manifest, source):
    expected = []
    for name, constant in (("native/veracrypt/utf8-console.patch", "COMMON_PATCH_SHA256"),
                           ("native/bsd-veracrypt/auxiliary-path.patch", "PATCH_SHA256")):
        actual = hashlib.sha256(source.read(name)).hexdigest()
        if source.constant("scripts/build-bsd-backends.py", constant) != actual:
            fail("BSD VeraCrypt source patch differs from its reviewed digest: " + name)
        expected.append({"path": name, "sha256": actual})
    if manifest.get("patches") != expected:
        fail("BSD VeraCrypt artifact patch provenance differs from exact source")


def verify_folder_patches(kind, manifest, source):
    paths = source.constant("scripts/build-folder-backends.py", "PATCHES").get(kind, [])
    expected = {name: hashlib.sha256(source.read(name)).hexdigest() for name in paths}
    if manifest.get("patches", {}) != expected:
        fail("folder artifact patch provenance differs from exact source: " + kind)


def verify_cryptomator_sources(root, source):
    # The helper is Lurker code, so a dependency lock alone cannot identify its
    # runtime behavior. Documentation changes do not require rebuilding Java.
    for name in ("src/main/java/org/lurker/cryptomator/Main.java", "src/main/java/module-info.java", "pom.xml"):
        packaged = root / "source" / name
        expected = hashlib.sha256(source.read("native/folder/cryptomator/" + name)).hexdigest()
        if not packaged.is_file() or packaged.is_symlink() or digest(packaged) != expected:
            fail("Cryptomator helper source differs from exact Lurker commit: " + name)


# These components belong to the host ABI and must never be shipped privately.
LINUX_SYSTEM_ELF = re.compile(r"^(?:ld(?:-linux[^/]*|64)?\.so(?:\..*)?|lib(?:c|m|pthread|dl|rt|resolv|util|anl|nss_[^/]+|BrokenLocale)\.so(?:\..*)?)$")
LINUX_CXX_RUNTIME_ELF = re.compile(r"^lib(?:stdc\+\+|gcc_s)\.so(?:\..*)?$")


def verify_linux_static_wx(root, manifest, source):
    records = manifest.get("static_libraries", [])
    if len(records) != 1 or records[0].get("name") != "wxWidgets":
        fail("Linux helpers require pinned private static wxWidgets provenance")
    if any(item.get("soname", "").startswith("libwx") for item in manifest.get("libraries", [])):
        fail("Linux private wxWidgets must be linked statically")
    record = records[0]
    name = "licenses/wxwidgets/provenance.json"
    if record.get("provenance") != name or name not in manifest["files"]:
        fail("Linux static wxWidgets provenance inventory is missing")
    verify_files(root, {name: manifest["files"][name]})
    receipt = json_file(root / name)
    inputs = receipt.get("inputs", {})
    constants = {key: source.constant("scripts/linux_wx.py", key) for key in
                 ("VERSION", "URL", "SHA256", "FLAGS", "LICENSES", "ENV_FLAGS")}
    if receipt.get("schema") != 1 or any(inputs.get(key) != constants[constant] for key, constant in
            (("version", "VERSION"), ("url", "URL"), ("archive_sha256", "SHA256"))):
        fail("Linux static wxWidgets source pin differs from selected source")
    flags = inputs.get("flags", [])
    if not flags or not flags[0].startswith("--prefix=/") or flags[1:] != list(constants["FLAGS"]):
        fail("Linux static wxWidgets configuration differs from selected source")
    notices = ["licenses/wxwidgets/" + str(relative(path)) for path in constants["LICENSES"]]
    if sorted(record.get("license_files", [])) != sorted(notices) or sorted(receipt.get("license_files", [])) != sorted(notices):
        fail("Linux static wxWidgets license list differs from selected source")
    verify_files(root, {path: manifest["files"].get(path, {}) for path in notices})
    major_minor = ".".join(constants["VERSION"].split(".")[:2])
    expected = {"lib/libwx_baseu-" + major_minor + ".a", "lib/libwxregexu-" + major_minor + ".a"}
    libraries = receipt.get("static_libraries", {})
    if set(libraries) != expected or any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in libraries.values()):
        fail("Linux static wxWidgets archive identities are missing")
    if not re.fullmatch(r"[0-9a-f]{64}", inputs.get("source_tree_sha256", "")) or not Path(inputs.get("source_dir", "")).is_absolute():
        fail("Linux static wxWidgets source-tree identity is missing")
    compilers = inputs.get("compilers", {})
    if set(compilers) != {"CC", "CXX"} or any(not Path(item.get("path", "")).is_absolute() or not item.get("version") or
            not re.fullmatch(r"[0-9a-f]{64}", item.get("sha256", "")) for item in compilers.values()):
        fail("Linux static wxWidgets compiler identity is missing")
    if set(inputs.get("environment", {})) != set(constants["ENV_FLAGS"]):
        fail("Linux static wxWidgets compiler flag provenance is missing")


def verify_linux_private_libraries(root, kind):
    manifest = json_file(root / "private-libraries.json")
    if manifest.get("schema") != 1 or not isinstance(manifest.get("files"), dict):
        fail("Linux private library manifest schema/inventory is missing")
    verify_files(root, manifest["files"])
    actual = {str(path.relative_to(root)) for path, _ in entries(root / "lib")}
    recorded = {name for name in manifest["files"] if name.startswith("lib/")}
    if actual != recorded:
        fail("Linux private library inventory has unmanifested/missing files")
    if any(not name.startswith(("lib/", "licenses/")) for name in manifest["files"]):
        fail("Linux private library inventory contains unrelated paths")
    readelf = shutil.which("readelf") or shutil.which("llvm-readelf")
    if not readelf:
        fail("readelf/llvm-readelf is required for private libraries")
    names = set()
    libraries = manifest.get("libraries", [])
    for library in libraries:
        name = str(relative(library["name"]))
        if "/" in name or name in names or LINUX_SYSTEM_ELF.fullmatch(name):
            fail("forbidden/duplicate Linux private library: " + name)
        names.add(name)
        path = root / "lib" / name
        if path.is_symlink() or digest(path) != library.get("sha256"):
            fail("Linux private library checksum/regular-file identity differs: " + name)
        if not library.get("source_path") or not Path(library["source_path"]).is_absolute():
            fail("Linux private library source provenance is missing: " + name)
        notices = library.get("license_files", [])
        if not notices or any(not str(relative(notice)).startswith("licenses/") or notice not in manifest["files"] for notice in notices):
            fail("Linux private library license inventory is missing: " + name)
        info = elf_info(path, readelf)
        if info is None or info["soname"] != [library.get("soname")] or library.get("soname") != name:
            fail("Linux private library SONAME identity differs: " + name)
        if info["search"] != ["$ORIGIN"] or info["interpreters"]:
            fail("Linux private library needs an exact $ORIGIN search path: " + name)
    if recorded != {"lib/" + name for name in names}:
        fail("Linux private library inventory differs from its library records")
    for name in ENTRY[kind] + ["lib/" + name for name in sorted(names)]:
        info = elf_info(root / name, readelf)
        if info is None:
            fail("Linux private closure contains a non-ELF file: " + name)
        if not name.startswith("lib/") and info["search"] != ["$ORIGIN/lib"]:
            fail("Linux helper needs an exact $ORIGIN/lib search path: " + name)
        for dependency in info["needed"]:
            if not (LINUX_SYSTEM_ELF.fullmatch(dependency) or LINUX_CXX_RUNTIME_ELF.fullmatch(dependency)) and dependency not in names:
                fail("Linux private closure is missing dependency " + dependency + " for " + name)
    return manifest


def validate_backend(kind, directory, source):
    root = safe_root(directory)
    list(entries(root))
    for name in ENTRY[kind]:
        require_executable(root / name)
        if kind != "cryptomator" and magic(root / name) != b"\x7fELF":
            fail("backend entry point is not a native ELF executable: " + name)
    if not (root / "licenses").is_dir() or not any(p.is_file() for p in (root / "licenses").rglob("*")):
        fail("backend licenses/notices are missing: " + kind)
    if kind in {"gocryptfs", "cryfs"}:
        manifest = json_file(root / (kind + ".json"))
        verify_platform(manifest)
        pin = source.constant("scripts/build-folder-backends.py", "PINS")[kind]
        if manifest.get("revision") != pin[1] or manifest.get("source") != pin[0] or digest(root / kind) != manifest.get("sha256"):
            fail("folder artifact source/binary differs from its pinned build manifest")
        verify_folder_patches(kind, manifest, source)
        license_files = manifest.get("license_files")
        if not license_files:
            fail("folder artifact license inventory is missing")
        verify_files(root / "licenses" / kind, license_files)
    elif kind == "cryptomator":
        manifest = json_file(root / "manifest.json")
        verify_platform(manifest)
        verify_files(root, manifest["files"])
        actual = {str(p.relative_to(root)) for p, _ in entries(root)} - {"manifest.json"}
        if actual != set(manifest["files"]):
            fail("Cryptomator payload contains unmanifested/missing files")
        verify_cryptomator_sources(root, source)
        lock = json.loads(source.read("native/folder/cryptomator/dependencies.lock.json"))
        if manifest.get("sources") != lock:
            fail("Cryptomator dependency provenance differs from source lock")
        if not list((root / "mods").glob("*.jar")) or not (root / "runtime/lib").is_dir():
            fail("Cryptomator private Java runtime/modules are incomplete")
    elif kind in {"veracrypt-create", "veracrypt-userspace"}:
        manifest = json_file(root / "creator-source.json")
        pin = source.constant("scripts/build-linux-userspace.py", "SOURCES")
        patch = source.read("native/linux-userspace/patches/veracrypt-create.patch")
        if tuple(manifest.get("source", [])) != tuple(pin["veracrypt-create"]) or manifest.get("patch_sha256") != hashlib.sha256(patch).hexdigest():
            fail("VeraCrypt creator source/patch provenance differs")
        archives = source.constant("scripts/build-linux-userspace.py", "SOURCE_ARCHIVE_SHA256")
        if manifest.get("source_archive_sha256") != archives["veracrypt-create"]:
            fail("VeraCrypt creator source archive checksum differs")
        if manifest.get("binary_sha256") != digest(root / "lurker-veracrypt-create"):
            fail("VeraCrypt creator binary checksum differs")
        private_manifest = verify_linux_private_libraries(root, kind)
        verify_linux_static_wx(root, private_manifest, source)
        if kind == "veracrypt-userspace":
            if json_file(root / "sources.json") != {key: list(value) for key, value in pin.items()}:
                fail("Linux userspace source pins differ")
            if json_file(root / "source-archives.json") != archives:
                fail("Linux userspace source archive checksums differ")
            envelope = json_file(root / "artifact-manifest.json")
            verify_envelope(envelope, root, source, kind)
    else:
        manifest = json_file(root / "bsd-veracrypt-source.json")
        verify_platform(manifest)
        verify_bsd_patches(manifest, source)
        pins = source.constant("scripts/build-bsd-backends.py", "PINS")
        actual_pins = manifest.get("sources", {})
        expected_pins = {key: value for key, value in pins.items() if key != "libiconv" or platform.system() == "FreeBSD"}
        if actual_pins != expected_pins or manifest.get("binary") != "lurker-veracrypt-bsd":
            fail("BSD VeraCrypt source pins or executable identity differs")
        if manifest.get("binary_sha256") != digest(root / "lurker-veracrypt-bsd"):
            fail("BSD VeraCrypt binary checksum differs")
        for library in manifest.get("private_libraries", []):
            name = str(relative(library["name"]))
            if "/" in name or digest(root / "lib" / name) != library["sha256"]:
                fail("BSD VeraCrypt private library checksum differs")
        if not manifest.get("files"):
            fail("BSD VeraCrypt complete runtime file inventory is missing")
        verify_files(root, manifest["files"])
        runtime_files = {"lurker-veracrypt-bsd"} | {str(p.relative_to(root)) for p, _ in entries(root)
                         if str(p.relative_to(root)).startswith(("lib/", "licenses/"))}
        if set(manifest["files"]) != runtime_files:
            fail("BSD VeraCrypt runtime inventory has unmanifested/missing assets")
    return root


def verify_envelope(manifest, root, source, kind):
    verify_platform(manifest)
    if manifest.get("backend") != kind or manifest.get("source_commit") != source.commit:
        fail("native artifact envelope identity differs")
    verify_files(root, manifest["files"])
    if set(manifest["files"]) != {str(p.relative_to(root)) for p, _ in entries(root)} - {"artifact-manifest.json"}:
        fail("native payload contains unmanifested/missing files")


def elf_info(path, readelf):
    with path.open("rb") as stream:
        header = stream.read(20)
    if not header.startswith(b"\x7fELF"):
        return None
    if len(header) < 20 or header[4] != 2 or header[5] not in (1, 2):
        fail("malformed or unsupported ELF header: " + str(path))
    allowed_abi = {"Linux": {0, 3}, "FreeBSD": {9}, "NetBSD": {0, 2}, "OpenBSD": {12}, "DragonFly": {0, 9}}
    if header[7] not in allowed_abi.get(platform.system(), set()):
        fail("ELF OS ABI differs: " + str(path))
    machine = int.from_bytes(header[18:20], "little" if header[5] == 1 else "big")
    expected = {"x86_64": 62, "aarch64": 183}.get(architecture(platform.machine()))
    if machine != expected:
        fail("ELF architecture differs: " + str(path))
    result = subprocess.run([readelf, "--wide", "-l", "-d", "--version-info", str(path)], capture_output=True, text=True, timeout=30,
                            env=dict(os.environ, LC_ALL="C"))
    if result.returncode:
        fail("cannot inspect ELF metadata: " + str(path))
    text = result.stdout
    needed = re.findall(r"\(NEEDED\).*?\[([^]]+)\]", text)
    paths = re.findall(r"\((?:RUNPATH|RPATH)\).*?\[([^]]*)\]", text)
    interpreters = re.findall(r"Requesting program interpreter:\s*([^]]+)\]", text)
    versions, definitions = parse_elf_versions(text)
    return {"needed": needed, "search": [p for entry in paths for p in entry.split(":")], "interpreters": interpreters,
            "soname": re.findall(r"\(SONAME\).*?\[([^]]+)\]", text),
            "version_needs": versions, "version_definitions": definitions}


def parse_elf_versions(text):
    needs, definitions, section, provider = {}, set(), None, None
    for line in text.splitlines():
        if line.startswith("Version "):
            section = "needs" if line.startswith("Version needs section") else "definitions" if line.startswith("Version definition section") else None
            provider = None
        if section == "needs":
            match = re.search(r"\bFile:\s+(\S+)", line)
            if match:
                provider = match[1]
                needs.setdefault(provider, set())
            match = re.search(r"\bName:\s+(\S+)", line)
            if match:
                if provider is None:
                    fail("ELF symbol version has no provider")
                needs[provider].add(match[1])
        elif section == "definitions":
            match = re.search(r"\bName:\s+(\S+)", line)
            if match:
                definitions.add(match[1])
    return {name: sorted(values) for name, values in needs.items()}, sorted(definitions)


def verify_elf_versions(consumer, name, target, target_info):
    required = consumer.get("version_needs", {}).get(name, [])
    missing = set(required) - set(target_info.get("version_definitions", []))
    if missing:
        fail("shared library " + str(target) + " lacks required symbol versions: " + ", ".join(sorted(missing)))
    return required


def private_jre_preload(path, name, prefix, readelf):
    # JLI loads the server VM before Java opens these native java.base libraries.
    # This is a specific loader relationship, not a search of every packaged lib.
    runtime = prefix / "libexec/lurker/backends/cryptomator/runtime"
    consumers = {"libjava.so", "libzip.so", "libnet.so", "libverify.so", "libjimage.so"}
    if name != "libjvm.so" or path.parent != runtime / "lib" or path.name not in consumers:
        return None
    backend_manifest = json_file(runtime.parent / "manifest.json")
    if backend_manifest.get("backend") != "cryptomator" or backend_manifest.get("protocol") != 1:
        fail("private Java preload requires the verified Cryptomator runtime manifest")
    verify_platform(backend_manifest)
    for anchor in (runtime / "bin/java", runtime / "lib/libjli.so", runtime / "lib/server/libjvm.so"):
        if not anchor.is_file() or magic(anchor) != b"\x7fELF":
            fail("private Java preload anchor is missing: " + str(anchor))
    configuration = runtime / "lib/jvm.cfg"
    if not configuration.is_file() or not re.search(r"(?m)^-server\s+KNOWN\s*$", configuration.read_text()):
        fail("private Java runtime does not qualify the server JVM loader path")
    jvm = runtime / "lib/server/libjvm.so"
    if elf_info(jvm, readelf).get("soname") != ["libjvm.so"]:
        fail("private server JVM has an unexpected SONAME")
    return jvm


def audit_dependencies(prefix):
    readelf = shutil.which("readelf") or shutil.which("llvm-readelf")
    if not readelf:
        fail("readelf/llvm-readelf is a packaging prerequisite; ldd is deliberately not used")
    defaults = [Path(p) for p in ("/lib", "/lib64", "/usr/lib", "/usr/lib64", "/usr/local/lib", "/usr/pkg/lib")]
    defaults += list(Path("/lib").glob("*-linux-gnu")) + list(Path("/usr/lib").glob("*-linux-gnu"))
    cache = {}
    ldconfig = shutil.which("ldconfig")
    if ldconfig:
        result = subprocess.run([ldconfig, "-p" if platform.system() == "Linux" else "-r"], capture_output=True, text=True, timeout=30)
        for name, path in re.findall(r"(\S+)\s+[^\n]*?=>\s+(/\S+)", result.stdout):
            cache.setdefault(name, Path(path))
    metadata = {}
    def inspect(path):
        real = path.resolve()
        if real not in metadata:
            metadata[real] = elf_info(path, readelf)
        return metadata[real]
    pending = [path for path, kind in entries(prefix) if kind == "file"]
    records, seen = [], set()
    while pending:
        path = pending.pop()
        real = path.resolve()
        if real in seen:
            continue
        seen.add(real)
        info = inspect(path)
        if info is None:
            if path.suffix == ".jar" and path.is_relative_to(prefix):
                with zipfile.ZipFile(path) as jar, tempfile.TemporaryDirectory(prefix="lurker-jar-audit-") as temporary:
                    for member in jar.infolist():
                        if member.is_dir():
                            continue
                        relative(member.filename)
                        with jar.open(member) as stream:
                            header = stream.read(20)
                        if not header.startswith(b"\x7fELF"):
                            continue
                        machine = int.from_bytes(header[18:20], "little" if header[5] == 1 else "big")
                        target_machine = {"x86_64": 62, "aarch64": 183}.get(architecture(platform.machine()))
                        if machine != target_machine:
                            continue
                        if member.file_size > 128 * 1024 * 1024:
                            fail("oversized embedded native library: " + member.filename)
                        extracted = Path(temporary) / Path(member.filename).name
                        with jar.open(member) as stream, extracted.open("wb") as output:
                            shutil.copyfileobj(stream, output)
                        nested = elf_info(extracted, readelf)
                        if nested["search"]:
                            fail("embedded native library needs an unqualified private search path: " + member.filename)
                        refs = []
                        for name in nested["needed"]:
                            target = next((d / name for d in defaults if (d / name).is_file()), cache.get(name))
                            if target is None or not target.is_file():
                                fail("unresolved embedded shared library " + name + " in " + str(path))
                            target_info = inspect(target)
                            if target_info is None:
                                fail("shared library is not ELF: " + str(target))
                            required = verify_elf_versions(nested, name, target, target_info)
                            pending.append(target)
                            refs.append({"name": name, "path": str(target.resolve()), "bundled": False,
                                         "sha256": digest(target), "required_versions": required})
                        records.append({"file": str(path.relative_to(prefix)) + "!" + member.filename,
                                        "bundled": True, "dependencies": refs})
            continue
        if platform.system() == "Linux" and path.resolve().is_relative_to(prefix.resolve()):
            if any(LINUX_SYSTEM_ELF.fullmatch(name) for name in [path.name] + info.get("soname", [])):
                fail("host glibc/system loader cannot be bundled: " + str(path))
        search = []
        for item in info["search"]:
            if not item:
                fail("unsafe empty ELF library search path: " + str(path))
            expanded = item.replace("${ORIGIN}", str(path.parent)).replace("$ORIGIN", str(path.parent))
            directory = Path(expanded)
            if "$" in expanded or not directory.is_absolute():
                fail("unsupported/relative ELF search path: " + item)
            if not directory.resolve().is_relative_to(prefix.resolve()) and not any(directory.resolve().is_relative_to(base.resolve()) for base in defaults):
                fail("developer/untrusted ELF search path: " + item)
            search.append(directory)
        dependencies = []
        for name in info["needed"] + info["interpreters"]:
            preloaded = private_jre_preload(path, name, prefix, readelf)
            if preloaded is not None:
                target = preloaded
            elif "/" in name:
                target = Path(name)
                if not target.is_absolute():
                    fail("relative ELF dependency: " + name)
            else:
                target = next((d / name for d in search + defaults if (d / name).is_file()), cache.get(name))
            if target is None or not target.is_file():
                fail("unresolved shared library " + name + " needed by " + str(path))
            target_info = inspect(target)
            if target_info is None:
                fail("shared library is not ELF: " + str(target))
            required = verify_elf_versions(info, name, target, target_info)
            bundled = target.resolve().is_relative_to(prefix.resolve())
            dependencies.append({"name": name, "path": str(target.relative_to(prefix)) if bundled else str(target.resolve()),
                                 "bundled": bundled, "sha256": digest(target), "required_versions": required})
            if preloaded is not None:
                dependencies[-1]["resolution"] = "preloaded-by-private-java-launcher"
                dependencies[-1]["runtime_validation"] = "packaged native Java/Cryptomator tests required"
            pending.append(target)
        records.append({"file": str(path.relative_to(prefix)) if path.is_relative_to(prefix) else str(path),
                        "bundled": path.is_relative_to(prefix), "dependencies": dependencies})
    return records


def record(args):
    source = Source(args.source_root, args.source_commit, args.source_manifest)
    source.verify_clean_build_source()
    root = safe_root(args.directory)
    if args.kind == "binaries":
        required = ["lurker", "lurker-helper"] + (["lurker-desktop"] if args.desktop else [])
        for name in required:
            require_executable(root / name)
        files = {name: {"sha256": digest(root / name)} for name in required}
    else:
        for name in ENTRY[args.kind]:
            require_executable(root / name)
        files = file_manifest(root, exclude=("artifact-manifest.json",))
    manifest = {"schema": 1, "source_commit": source.commit, "backend": args.kind,
                "os": platform.system(), "architecture": platform.machine(), "files": files}
    if args.upstream_revision:
        manifest["upstream_revision"] = args.upstream_revision
    with args.output.open("x") as stream:
        json.dump(manifest, stream, indent=2)
        stream.write("\n")
    return 0


def desktop_authorization(selected, desktop):
    privileged = set(selected) & {"luks", "luks1", "veracrypt", "geli", "cgd", "softraid", "raid1c", "pefs"}
    if not desktop or not privileged:
        return None
    search = os.environ.get("PATH", "") + ":/usr/local/bin:/usr/pkg/bin:/usr/local/sbin:/usr/pkg/sbin"
    available = {name: path for name in ("pkexec", "sudo") if (path := shutil.which(name, path=search))}
    if not available:
        fail("desktop native operations require host pkexec or sudo plus an administrator policy authorizing the installed lurker-helper; install/configure authorization before staging")
    return {"required_for": sorted(privileged), "alternatives": ["pkexec", "sudo"], "available": available,
            "policy_requirement": "working administrator authorization for the installed lurker-helper",
            "qualification": "executable presence checked only; authorization policy and login-user desktop operation require native testing"}


def stage(args):
    system = platform.system()
    if system not in SUPPORTED:
        fail("use native macOS packaging; supported staging targets are Linux and four BSDs")
    source = Source(args.source_root, args.source_commit, args.source_manifest)
    selected = set(args.select)
    if not selected:
        fail("select at least one backend explicitly; omission must not imply complete support")
    for name in selected:
        if name not in ALLOW or system not in ALLOW[name]:
            fail("backend/OS pair outside approved scope: " + name + "/" + system)
    artifacts = {}
    input_snapshots = {}
    for item in args.backend:
        name, separator, directory = item.partition("=")
        if not separator or name not in ARTIFACTS or name in artifacts:
            fail("invalid/duplicate backend artifact: " + item)
        if ARTIFACTS[name] not in selected:
            fail("artifact backend was not selected: " + name)
        if name.startswith("veracrypt-") and ((name == "veracrypt-bsd") == (system == "Linux")):
            fail("VeraCrypt artifact route does not match native OS")
        input_snapshots[name] = file_manifest(safe_root(directory))
        artifacts[name] = validate_backend(name, Path(directory), source)
    for name in selected & {"gocryptfs", "cryfs", "cryptomator"}:
        if name not in artifacts:
            fail("selected backend runtime missing: " + name)
    if "veracrypt" in selected and not any(name.startswith("veracrypt-") for name in artifacts):
        fail("selected VeraCrypt private runtime missing")
    if "veracrypt-userspace" in artifacts and "veracrypt-create" in artifacts:
        fail("creator is already part of userspace payload")
    binaries = safe_root(args.binaries)
    provenance = json_file(args.binary_manifest)
    verify_platform(provenance)
    if provenance.get("source_commit") != source.commit or provenance.get("backend") != "binaries" or provenance.get("schema") != 1:
        fail("compiled binaries source identity differs")
    required = {"lurker", "lurker-helper"} | ({"lurker-desktop"} if args.desktop else set())
    if not required <= set(provenance["files"]):
        fail("binary provenance misses required products")
    verify_files(binaries, {name: provenance["files"][name] for name in required})
    requirements = {}
    for backend in sorted(selected):
        for requirement in TOOLS.get(system, {}).get(backend, []):
            choices = requirement if isinstance(requirement, list) else [requirement]
            found = [(tool, shutil.which(tool, path=os.environ.get("PATH", "") + ":/sbin:/usr/sbin:/usr/local/sbin:/usr/pkg/sbin")) for tool in choices]
            match = next(((tool, path) for tool, path in found if path), None)
            if match is None:
                fail("selected backend host tool missing: " + " or ".join(choices))
            requirements[match[0]] = match[1]
    if "veracrypt-userspace" in artifacts:
        fuse = shutil.which("fusermount3")
        if not fuse:
            fail("userspace VeraCrypt requires the host fusermount3 helper")
        requirements["fusermount3"] = fuse
    authorization = desktop_authorization(selected, args.desktop)
    output = args.output.absolute()
    if ".." in output.parts or output.exists() or output.is_symlink():
        fail("output must not exist: " + str(output))
    parent = output.parent
    info = parent.lstat()
    if parent.is_symlink() or parent.resolve() != parent or not stat.S_ISDIR(info.st_mode) or info.st_uid != os.geteuid() or info.st_mode & 0o022:
        fail("output parent must be a real directory owned by current uid without group/world write")
    backing = Path(tempfile.mkdtemp(prefix="." + output.name + "-", dir=parent))
    published = False
    try:
        prefix = backing / "prefix"
        (prefix / "bin").mkdir(parents=True)
        private = prefix / "libexec/lurker/backends"
        private.mkdir(parents=True)
        share = prefix / "share/lurker"
        (share / "provenance").mkdir(parents=True)
        (share / "licenses").mkdir()
        for name in sorted(required):
            require_executable(binaries / name)
            with (binaries / name).open("rb") as stream:
                if stream.read(4) != b"\x7fELF":
                    fail("compiled product is not a native ELF executable: " + name)
            shutil.copyfile(binaries / name, prefix / "bin" / name)
            (prefix / "bin" / name).chmod(0o755)
        verify_files(prefix / "bin", {name: provenance["files"][name] for name in required})
        (share / "provenance/rust-binaries.json").write_text(json.dumps(provenance, indent=2) + "\n")
        (share / "licenses/LURKER-LICENSE").write_bytes(source.read("LICENSE"))
        for kind, directory in artifacts.items():
            snapshot = input_snapshots[kind]
            def copied_file(name, target):
                shutil.copyfile(directory / name, target)
                expected = snapshot[name].get("sha256")
                if not expected or digest(target) != expected:
                    fail("input changed during staging: " + name)
            def copied_tree(name, target):
                source_tree = directory / name if name else directory
                copy_tree(source_tree, target)
                prefix_name = name.rstrip("/") + "/" if name else ""
                expected = {key[len(prefix_name):]: value for key, value in snapshot.items() if key.startswith(prefix_name)}
                verify_files(target, expected)
            if kind == "cryptomator":
                copied_tree("", private / "cryptomator")
            elif kind in {"gocryptfs", "cryfs", "veracrypt-create"}:
                for name in ENTRY[kind]:
                    copied_file(name, private / name)
                    (private / name).chmod(0o755)
                metadata = kind + ".json" if kind in {"gocryptfs", "cryfs"} else "creator-source.json"
                copied_file(metadata, share / "provenance" / (kind + ".json"))
                licenses = "licenses/" + kind if kind in {"gocryptfs", "cryfs"} else "licenses"
                copied_tree(licenses, share / "licenses" / kind)
                if kind == "veracrypt-create":
                    copied_tree("lib", private / "lib")
                    copied_file("private-libraries.json", share / "provenance/veracrypt-create-libraries.json")
            else:
                for name in ENTRY[kind]:
                    copied_file(name, private / name)
                    (private / name).chmod(0o755)
                if (directory / "lib").is_dir():
                    copied_tree("lib", private / "lib")
                copied_tree("licenses", share / "licenses" / kind)
                metadata = ["bsd-veracrypt-source.json"] if kind == "veracrypt-bsd" else ["sources.json", "source-archives.json", "creator-source.json", "artifact-manifest.json", "private-libraries.json"]
                for name in metadata:
                    if (directory / name).is_file():
                        target = share / "provenance" / kind
                        target.mkdir(exist_ok=True)
                        copied_file(name, target / name)
        dependencies = audit_dependencies(prefix)
        manifest = {"schema": 1, "source_commit": source.commit, "os": system, "architecture": platform.machine(),
                    "selected_backends": sorted(selected), "bundled_artifacts": sorted(artifacts),
                    "host_tools": requirements, "native_dependencies": dependencies,
                    "desktop_authorization": authorization,
                    "host_tool_requirements": {backend: TOOLS.get(system, {}).get(backend, []) for backend in sorted(selected)},
                    "runtime_requirements": ["matching host kernel/filesystem and mount authorization"]
                        + (["Desktop native operations require host pkexec or sudo and working administrator policy for the installed lurker-helper"] if authorization else [])
                        + (["CryFS 1.x requires FUSE ABI 2 (libfuse.so.2 and fusermount)"] if "cryfs" in selected else [])
                        + (["gocryptfs requires FUSE; Linux accepts fusermount3 or fusermount, FreeBSD uses mount_fusefs"] if "gocryptfs" in selected else [])
                        + (["Cryptomator's qualified Linux adapter requires libfuse3 and fusermount3"] if "cryptomator" in selected else [])
                        + (["VeraCrypt userspace/LKL requires the FUSE kernel interface and fusermount3"] if "veracrypt-userspace" in artifacts else [])
                        + (["PEFS module must match the FreeBSD kernel"] if "pefs" in selected else []),
                    "self_contained": False, "native_operation_qualification": "not performed by staging",
                    "files": file_manifest(prefix)}
        (share / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        archive = prefix / "package.tar.gz"
        def archive_filter(info):
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            if info.isdir():
                info.mode = 0o755
            return info
        with tarfile.open(archive, "w:gz", dereference=False) as tar:
            for child in sorted(prefix.iterdir()):
                if child != archive:
                    tar.add(child, arcname="lurker/" + child.name, filter=archive_filter)
        archive_digest = digest(archive)
        (prefix / "package.sha256").write_text(archive_digest + "  package.tar.gz\n")
        # symlink(2) is atomic and fails for every existing destination type,
        # including an empty directory. Keep the completed backing tree on success.
        output.symlink_to(os.path.relpath(prefix, parent), target_is_directory=True)
        published = True
        print(json.dumps({"status": "staged", "prefix": str(output), "archive": str(output / "package.tar.gz"),
                          "archive_sha256": archive_digest, "source_commit": source.commit,
                          "selected_backends": sorted(selected)}))
    finally:
        if not published:
            shutil.rmtree(backing)
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("record", "stage"):
        cmd = sub.add_parser(name)
        cmd.add_argument("--source-root", type=Path, default=ROOT)
        cmd.add_argument("--source-commit", required=True)
        cmd.add_argument("--source-manifest", type=Path)
        cmd.add_argument("--output", type=Path, required=True)
        cmd.add_argument("--desktop", action="store_true")
    record_parser = sub.choices["record"]
    record_parser.add_argument("--kind", choices=["binaries", *ARTIFACTS], required=True)
    record_parser.add_argument("--directory", type=Path, required=True)
    record_parser.add_argument("--upstream-revision")
    stage_parser = sub.choices["stage"]
    stage_parser.add_argument("--binaries", type=Path, required=True)
    stage_parser.add_argument("--binary-manifest", type=Path, required=True)
    stage_parser.add_argument("--select", action="append", default=[])
    stage_parser.add_argument("--backend", action="append", default=[])
    args = parser.parse_args()
    try:
        return record(args) if args.command == "record" else stage(args)
    except (ValueError, OSError, KeyError, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        print(json.dumps({"status": "blocked", "error": str(error)}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
