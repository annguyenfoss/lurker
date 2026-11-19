import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location("build_bsd_package", SCRIPTS / "build-bsd-package.py")
BSD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BSD)


class PackagePayloadTests(unittest.TestCase):
    payload = {"bin/lurker": b"native binary", "share/lurker/manifest.json": b"staged manifest"}
    manifest = {"files": {
        "bin/lurker": {"sha256": hashlib.sha256(payload["bin/lurker"]).hexdigest(), "executable": True},
        "bin/lurker-link": {"symlink": "lurker"},
    }}
    manifest_digest = hashlib.sha256(payload["share/lurker/manifest.json"]).hexdigest()

    def verify(self, *, use_pkg=False, omit=None, path_override=None, extra_mode=0, link="lurker", user="root"):
        with tempfile.TemporaryDirectory() as work:
            path = Path(work) / "package.tgz"
            with tarfile.open(path, "w:gz") as archive:
                for name in sorted(set(self.payload) | {"bin/lurker-link"}):
                    if name == omit:
                        continue
                    ident = "/usr/local/" + name if use_pkg else name
                    if path_override and name == "bin/lurker":
                        ident = path_override
                    entry = tarfile.TarInfo(ident)
                    entry.uid = entry.gid = 1300 if use_pkg else 0
                    entry.uname, entry.gname = user, "wheel"
                    entry.mode = (0o755 | extra_mode) if name == "bin/lurker" else 0o644
                    if name == "bin/lurker-link":
                        entry.type, entry.linkname = tarfile.SYMTYPE, link
                        archive.addfile(entry)
                    else:
                        data = self.payload[name]
                        entry.size = len(data)
                        archive.addfile(entry, io.BytesIO(data))
            BSD.verify_package(path, "/usr/local", self.manifest, self.manifest_digest, use_pkg)

    def test_pkg_uses_named_owners_and_pkg_create_uses_root_ids(self):
        self.verify()
        self.verify(use_pkg=True)
        with self.assertRaisesRegex(ValueError, "owner"):
            self.verify(use_pkg=True, user="adjutant")

    def test_package_cannot_omit_staged_payload(self):
        with self.assertRaisesRegex(ValueError, "omitted"):
            self.verify(omit="bin/lurker")

    def test_package_cannot_relocate_or_change_link(self):
        with self.assertRaisesRegex(ValueError, "entry"):
            self.verify(use_pkg=True, path_override="/usr/local/sbin/lurker")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.verify(link="/etc/passwd")

    def test_package_cannot_add_setuid_or_writable_mode(self):
        for mode in (0o4000, 0o022):
            with self.subTest(mode=mode), self.assertRaisesRegex(ValueError, "permissions"):
                self.verify(extra_mode=mode)
