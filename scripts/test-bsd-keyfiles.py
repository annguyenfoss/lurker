#!/usr/bin/env python3
"""Binary-keyfile CLI conformance for GELI, PEFS and DragonFly LUKS1.

Reuse the qualified native CLI/PTY workload suite. Only fresh private children
of /tmp are used. Full evidence is retained; no forced unmount, native detach,
repair, package installation or automatic recovery is performed by this script.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import stat
import subprocess
import sys

SPEC = importlib.util.spec_from_file_location("keyfile_conformance", Path(__file__).with_name("test-veracrypt-keyfiles.py"))
KEYFILES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(KEYFILES)
BASE = KEYFILES.conformance
BACKENDS = {"FreeBSD": {"geli", "pefs"}, "DragonFly": {"luks1"}}


def private_binary(binary):
    for path in [binary, *binary.parents]:
        info = path.stat()
        sticky_tmp = path == Path("/tmp") and info.st_mode & stat.S_ISVTX
        if info.st_uid != 0 or (info.st_mode & 0o022 and not sticky_tmp):
            raise ValueError(f"Root tests require an administrator-controlled binary and ancestors: {path}")
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("--binary must name an executable artifact")


def owned_path(value, root):
    if isinstance(value, dict):
        return any(owned_path(item, root) for item in value.values())
    if isinstance(value, list):
        return any(owned_path(item, root) for item in value)
    return isinstance(value, str) and value.startswith("/") and Path(os.path.normpath(value)).is_relative_to(root)


def clear_overrides():
    names = {"LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES",
             "SUDO_UID", "SUDO_GID", "PKEXEC_UID", "DOAS_USER"}
    for name in list(os.environ):
        if name.startswith("LURKER_") or name in names:
            os.environ.pop(name, None)


class BSDKeyfileSuite(KEYFILES.KeyfileSuite):
    def __init__(self, options):
        super().__init__(options)
        # Guarantee binary delimiters instead of relying on random occurrence.
        # These are only our freshly created private keyfiles, never user files.
        for path in [self.keyfile, self.wrong_keyfile]:
            descriptor = os.open(path, os.O_WRONLY | os.O_TRUNC | os.O_NOFOLLOW)
            with os.fdopen(descriptor, "wb") as stream:
                stream.write(b"\0\xff\n\r" + os.urandom(252))
                stream.flush()
                os.fsync(stream.fileno())
        self.result.update(suite=f"{platform.system().lower()}-binary-keyfile-cli", harness_sha256=BASE.file_hash(Path(__file__)),
                           parent_harness_sha256=BASE.file_hash(Path(BASE.__file__)),
                           credential_harness_sha256=BASE.file_hash(Path(KEYFILES.__file__)),
                           keyfile_prefix="NUL/FF/LF/CR; 256 bytes", no_password_prompts=True,
                           limits="No VM shutdown, forced detach, repair, desktop or external dependency qualification")
        self.baseline = None
        self.save()

    @staticmethod
    def native(argv):
        result = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
        if result.returncode or len(result.stdout) > 65536 or len(result.stderr) > 65536:
            raise AssertionError("Native resource query failed or exceeded bounded output")
        return result.stdout.decode("utf-8", "strict")

    def records(self):
        directory = Path("/var/run/lurker-bsd")
        if not directory.exists():
            return {}
        records = {}
        for path in directory.glob("*.json"):
            info = path.lstat()
            if not stat.S_ISREG(info.st_mode) or info.st_size > 1024 * 1024:
                raise AssertionError("Unknown ownership journal; preserve it for inspection")
            value = json.loads(path.read_text())
            if owned_path(value, self.root):
                records[path.name] = value
        return records

    def footprint(self):
        footprint = {"mounts": self.native(["/sbin/mount", "-p"]), "sessions": self.records()}
        if self.options.backend == "luks1":
            footprint.update(vnodes=self.native(["/usr/sbin/vnconfig", "-l"]),
                             mappings=self.native(["/sbin/dmsetup", "ls"]))
        else:
            footprint.update(md=self.native(["/sbin/mdconfig", "-l"]),
                             geli=self.native(["/sbin/geli", "status"]))
        return footprint

    def cli(self, args, expected=True, password=None):
        before = self.footprint() if not expected else None
        result = super().cli(args, expected=expected, password=password)
        if before is not None and self.footprint() != before:
            raise AssertionError("Rejected CLI operation changed native resources or its journal; fixture retained")
        return result

    def create(self):
        self.baseline = self.footprint()
        super().create()
        expected_mode = 0o700 if self.options.backend == "pefs" else 0o600
        self.private_owned(self.source, expected_mode)
        if self.options.backend == "pefs":
            self.private_owned(self.source / ".pefs.db", 0o600)
        if self.footprint() != self.baseline:
            raise AssertionError("Create left native resources or an incomplete ownership journal")
        self.result["published_permissions"] = {"uid": os.geteuid(), "gid": os.getegid(),
                                                "mode": oct(expected_mode)}
        self.save()

    @staticmethod
    def private_owned(path, mode):
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode) or (info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode)) != (os.geteuid(), os.getegid(), mode):
            raise AssertionError("Lurker did not publish private storage owned by its invoking account")

    def mount(self, password=None, readonly=False, target=None, expected=True):
        super().mount(password=password, readonly=readonly, target=target, expected=expected)
        if expected and len(self.records()) != 1:
            raise AssertionError("Successful mount lacks exactly one recoverable owned session")

    def incorrect(self):
        before = BASE.snapshot(self.source)
        super().incorrect()
        if BASE.snapshot(self.source) != before:
            raise AssertionError("Incorrect binary key changed encrypted bytes or source identity")

    def close(self):
        super().close()
        if self.footprint() != self.baseline:
            raise AssertionError("Close leaked a native resource or retained a completed ownership journal")


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--backend", choices=["geli", "luks1", "pefs"], required=True)
    parser.add_argument("--cipher", choices=["aes", "serpent", "twofish"],
                        help="LUKS1 create cipher (default AES)")
    parser.add_argument("--commit", required=True, help="Verified exact full source Git SHA of the supplied CLI")
    parser.add_argument("--root", type=Path, default=Path("/tmp"))
    parser.add_argument("--size-gb", default="0.125")
    parser.add_argument("--timeout", type=float, default=180)
    options = parser.parse_args(argv)
    if options.backend not in BACKENDS.get(platform.system(), set()) or os.geteuid() != 0:
        parser.error("Run as root on the designated FreeBSD or DragonFly test guest with a native backend")
    if options.cipher is not None and options.backend != "luks1":
        parser.error("--cipher is only applicable to DragonFly LUKS1")
    if not re.fullmatch(r"[0-9a-f]{40}", options.commit) or not 0 < options.timeout <= 1800:
        parser.error("Use a full lowercase source SHA and a positive bounded timeout")
    from decimal import Decimal, InvalidOperation
    try:
        size = Decimal(options.size_gb)
        if not size.is_finite() or not Decimal("0.0625") <= size <= Decimal("1"):
            raise InvalidOperation
    except InvalidOperation:
        parser.error("Disposable file images must be between 0.0625 and 1 GiB")
    options.root = options.root.resolve(strict=True)
    if not options.root.is_dir() or not options.root.is_relative_to(Path("/tmp")):
        parser.error("--root must name an existing directory beneath /tmp")
    options.binary = options.binary.resolve(strict=True)
    private_binary(options.binary)
    if options.backend in {"geli", "luks1"} and options.cipher is None:
        options.cipher = "aes"
    options.auto_detect = True
    options.unprivileged = False
    options.expect_readonly_unsupported = False
    options.skip_readonly = None
    return options


def main(argv=None):
    try:
        options = parse_args(argv)
        clear_overrides()
        suite = BSDKeyfileSuite(options)
        result = suite.run()
        print(json.dumps({"schema": 1, "suite": result["suite"], "status": result["status"],
                          "backend": result["backend"], "authentication": result["authentication"],
                          "source_commit": result["source_commit"], "counts": result["counts"],
                          "duration_seconds": result["duration_seconds"], "result": str(suite.root / "result.json"),
                          "diagnostic": result.get("diagnostic")}))
        return 0 if result["status"] == "passed" else 1
    except (OSError, ValueError) as error:
        print(json.dumps({"schema": 1, "status": "blocked", "diagnostic": str(error)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
