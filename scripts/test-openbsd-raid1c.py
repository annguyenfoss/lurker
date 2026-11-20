#!/usr/bin/env python3
"""Actual CLI RAID1C conformance on fresh private /tmp file members.

Run as root on a disposable OpenBSD guest with an exact CLI artifact and source
SHA. Requires test-backends.py beside this script for its qualified PTY/password
transport and application workload helpers. Never repairs, rebuilds, forcibly
unmounts, or directly detaches native storage. Full logs and images are retained.
"""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import struct
import sys
import time

SPEC = importlib.util.spec_from_file_location("lurker_cli_conformance", Path(__file__).with_name("test-backends.py"))
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)


def arguments(binary, action, source, *, mountpoint=None, peer=None, kind=None,
              degraded=False, readonly=False, size_gb="0.125"):
    command = [str(binary), "--color=never", action, "--type", kind or ("raid1c" if action == "create" else "auto")]
    if peer is not None:
        command.extend(["--member", str(peer)])
    if degraded:
        command.append("--degraded")
    if readonly:
        command.append("--readonly")
    command.append(str(source))
    if action == "create":
        command.append(size_gb)
    elif action == "mount":
        if mountpoint is None:
            raise ValueError("Mount needs an owned mountpoint")
        command.append(str(mountpoint))
    return command


def member_parent(root, long_parent):
    # A120-character component is valid on the guest but exceeds the previous
    # creation-parent restriction; plaintext mountpoints stay short and separate.
    return root / ("members-" + "x" * 112) if long_parent else root


def owned_path(value, root):
    return isinstance(value, str) and value.startswith("/") and Path(os.path.normpath(value)).is_relative_to(root)


def contains_owned_path(value, root):
    if isinstance(value, dict):
        return any(contains_owned_path(v, root) for v in value.values())
    if isinstance(value, list):
        return any(contains_owned_path(v, root) for v in value)
    return owned_path(value, root)


def require_idle(state):
    if state["bioctl"].strip() or state["sessions"]:
        raise AssertionError("Native controller or owned journal is active; preserve it and obtain a quiet test window")
    for line in state["vnconfig"].splitlines():
        if not re.fullmatch(r"vnd\d+: not in use", line):
            raise AssertionError("Native vnode is active or unknown; preserve it and obtain a quiet test window")


def native_headers(paths):
    """Read bounded metadata of this suite's native whole-file RAID members.

    The qualified disklabel RAID* layout starts its chunk at file offset0;
    native softraid metadata begins8192 bytes into that chunk. No vnode needed.
    """
    result = {}
    for path in paths:
        with path.open("rb") as stream:
            stream.seek(8192)
            value = stream.read(168)
        if len(value) != 168 or struct.unpack_from("=Q", value, 0)[0] != 0x4d4152436372616d:
            raise AssertionError("Unexpected retained native member layout")
        number = lambda offset: struct.unpack_from("=I", value, offset)[0]
        if number(8) != 6 or number(32) != 2 or number(52) != 0x1c:
            raise AssertionError("Unexpected retained native mirror profile")
        result[path.name] = {"array_uuid": value[16:32].hex(), "members": number(32),
                             "member_index": number(36), "level": hex(number(52)),
                             "generation": struct.unpack_from("=Q", value, 152)[0]}
    return result


def private_binary(binary):
    """An explicit root-controlled executable; permit the sticky /tmp ancestor."""
    for path in [binary, *binary.parents]:
        info = path.stat()
        sticky_tmp = path == Path("/tmp") and info.st_mode & 0o1000
        if info.st_uid != 0 or (info.st_mode & 0o022 and not sticky_tmp):
            raise ValueError(f"Root tests require an administrator-controlled artifact and ancestors: {path}")
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise ValueError("--binary must identify an executable artifact")


class MirrorSuite(BASE.Suite):
    def __init__(self, options):
        super().__init__(options)
        self.member_parent = member_parent(self.root, options.long_parent)
        if self.member_parent != self.root:
            self.member_parent.mkdir(mode=0o700)
        self.source = self.member_parent / self.source.name
        self.peer = self.member_parent / "peer 密碼 with spaces.img"
        self.descriptor = Path(str(self.source) + ".raid1c.json")
        self.held = None
        self.result.update(suite="openbsd-raid1c-cli", harness_sha256=BASE.file_hash(Path(__file__)), profile="two members; native AES-XTS/bcrypt_pbkdf; FFS2",
                           size_gb_per_member=options.size_gb,
                           long_parent=options.long_parent, member_parent=str(self.member_parent),
                           actual_mountpoint=str(self.mountpoint),
                           limitations=["No device create, VM shutdown, rebuild, repair, forced detach or readonly native mapping"])
        self.save()

    def command(self, action, source=None, **options):
        return arguments(self.options.binary, action, source or self.source,
                         size_gb=self.options.size_gb, **options)

    def create_args(self):
        return self.command("create", peer=self.peer)

    def native(self, argv):
        result = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, timeout=15)
        if result.returncode or len(result.stdout) > 65536 or len(result.stderr) > 65536:
            raise AssertionError("Native state inspection failed or exceeded bounded output")
        return result.stdout.decode("utf-8", "replace")

    def records(self):
        directory = Path("/var/run/lurker-bsd")
        if not directory.exists():
            return {}
        result = {}
        for path in directory.glob("*.json"):
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 1024 * 1024:
                raise AssertionError("Unexpected native ownership journal; preserve for inspection")
            record = json.loads(path.read_text())
            if contains_owned_path(record, self.root):
                result[path.name] = record
        return result

    def footprint(self):
        return {"vnconfig": self.native(["/sbin/vnconfig", "-l"]),
                "bioctl": self.native(["/sbin/bioctl", "softraid0"]), "sessions": self.records()}

    def member_snapshot(self):
        paths = [self.source, self.peer, self.descriptor]
        if self.held is not None:
            paths.append(self.held)
        return {str(path): BASE.snapshot(path) for path in paths if path.exists()}

    def guarded_rejection(self, argv, *, password=None, unchanged_hash=True, diagnostic=None):
        before_state = self.footprint()
        before_files = self.member_snapshot() if unchanged_hash else None
        self.cli(argv, expected=False, password=password)
        if self.footprint() != before_state:
            raise AssertionError("Rejected command changed native resources or the ownership journal; retained")
        if before_files is not None and self.member_snapshot() != before_files:
            raise AssertionError("Rejected command changed member/descriptor bytes or identity; retained")
        if os.path.ismount(self.other_mount):
            raise AssertionError("Rejected command mounted the duplicate target; retained")
        if diagnostic:
            path = Path(self.result["cases"][-1]["commands"][-1]["log"])
            if diagnostic not in path.read_text():
                raise AssertionError(f"Expected actionable diagnostic {diagnostic!r}; {path}")

    def create(self):
        super().create()
        if not self.peer.is_file() or not self.descriptor.is_file():
            raise AssertionError("Create did not publish both members and its descriptor")
        value = json.loads(self.descriptor.read_text())
        if value.get("version") != 1 or value.get("volume_type") != "raid1c" or not re.fullmatch(r"[0-9a-f]{32}", value.get("array_uuid", "")):
            raise AssertionError("Generated mirror descriptor is invalid")
        members = [(self.descriptor.parent / p).resolve() for p in value.get("member_paths", [])]
        if members != [self.source, self.peer]:
            raise AssertionError("Descriptor does not identify both newly published member files")
        require_idle(self.footprint())

    def mount(self, password=None, readonly=False, target=None, expected=True,
              source=None, peer=None, kind=None, degraded=False):
        target = target or self.mountpoint
        self.cli(self.command("mount", source, mountpoint=target, peer=peer, kind=kind,
                              readonly=readonly, degraded=degraded), expected=expected,
                 password=password if password is not None else self.password)
        if expected:
            if not os.path.ismount(target):
                raise AssertionError("CLI succeeded without attaching its owned mountpoint")
            self.mounted = target == self.mountpoint
            if not self.records():
                raise AssertionError("Mounted mirror has no recoverable ownership journal")

    def close(self, target=None):
        self.cli(self.command("unmount", target or self.mountpoint))
        if os.path.ismount(self.mountpoint):
            raise AssertionError("CLI reported close but its owned mount remains")
        self.mounted = False
        require_idle(self.footprint())

    def absent_descriptor_diagnostic(self):
        held = self.descriptor.with_name("held descriptor.json")
        self.descriptor.rename(held)
        before = BASE.snapshot(held)
        try:
            self.guarded_rejection(self.command("mount", mountpoint=self.mountpoint), password=self.password,
                                   diagnostic="--type raid1c --member")
            if BASE.snapshot(held) != before:
                raise AssertionError("Rejected Auto command changed the retained descriptor")
        finally:
            if self.descriptor.exists():
                raise AssertionError("Rejected Auto command unexpectedly published a descriptor")
            held.rename(self.descriptor)

    def active_aliases(self):
        aliases = [self.root / "primary hardlink.img", self.root / "peer hardlink.img"]
        os.link(self.source, aliases[0]); os.link(self.peer, aliases[1])
        for primary, peer in [(self.source, self.peer), (aliases[0], self.peer), (self.source, aliases[1])]:
            self.guarded_rejection(self.command("mount", primary, peer=peer, kind="raid1c", mountpoint=self.other_mount),
                                   password=self.password, unchanged_hash=False)
            self.verify()

    def busy_session(self):
        before = self.footprint()
        super().busy()
        if self.footprint() != before or not self.records():
            raise AssertionError("Busy close changed the live mirror ownership/session")

    def degraded_status(self, case):
        values = list(self.records().values())
        if len(values) != 1 or not re.fullmatch(r"/dev/sd\d+a", values[0].get("mapped", "")):
            raise AssertionError("Cannot identify the owned degraded array in its journal")
        disk = values[0]["mapped"].removeprefix("/dev/")[:-1]
        listing = self.native(["/sbin/bioctl", disk])
        if "Degraded" not in listing or "Offline" not in listing or "RAID1C" not in listing:
            raise AssertionError("Expected native degraded mirror with an offline peer")
        self.result.setdefault("native_degraded_status", {})[case] = listing
        self.save()

    def degraded_survivor(self):
        self.held = self.root / "held missing primary.img"
        self.source.rename(self.held)
        self.guarded_rejection(self.command("mount", mountpoint=self.mountpoint), password=self.password)
        self.guarded_rejection(self.command("mount", kind="raid1c", peer=self.peer, degraded=True, mountpoint=self.mountpoint),
                               password=self.wrong)
        # Keep the original descriptor: it identifies a missing logical primary
        # and an explicitly supplied surviving peer, without inventing a file.
        self.mount(kind="raid1c", peer=self.peer, degraded=True)
        self.verify(); self.degraded_status("logical-primary-missing-explicit-peer")
        self.close(target=self.source)
        # The documented survivor-only recovery route also remains usable.
        self.mount(source=self.peer, kind="raid1c", degraded=True)
        self.verify(); self.degraded_status("secondary-survives-primary-missing"); self.close()
        self.held.rename(self.source); self.held = None
        # Qualified OpenBSD8 behavior: recovering lone secondary1 renumbers it
        # to index0. Returning stale primary0 conflicts (observed generations
        # 12 versus8); reject it without writes, never normalize native headers.
        # Native sr_meta_save assigns current chunk indices; bioctl(8) requires
        # a rebuild before missing members can rejoin:
        # https://man.openbsd.org/bioctl.8
        self.guarded_rejection(self.command("mount", kind="raid1c", peer=self.peer,
                                            degraded=True, mountpoint=self.mountpoint),
                               password=self.password, diagnostic="repeat a member index")
        self.result["limitations"].append("A recovered secondary may become native member0; returning stale primary0 is refused as a conflicting index. Continue explicit survivor recovery until a separate administrator rebuild; no automatic rejoin.")
        self.mount(source=self.peer, kind="raid1c", degraded=True)
        self.verify(); self.degraded_status("survivor-still-readable-after-conflicting-return-refused"); self.close()
        self.result["native_member_headers"] = native_headers([self.source, self.peer]); self.save()

    def normal_cleanup(self):
        errors = []
        for point in [self.other_mount, self.mountpoint]:
            if os.path.ismount(point):
                try:
                    self.cli(self.command("unmount", point))
                except Exception as error:
                    errors.append(str(error))
        # An acquisition may fail before filesystem attachment. Only ask the
        # CLI to close a journal whose recorded source belongs to this fixture.
        for record in self.records().values():
            source = record.get("source")
            if record.get("backend") == "raid1c" and owned_path(source, self.root):
                try:
                    self.cli(self.command("unmount", Path(source)))
                except Exception as error:
                    errors.append(str(error))
        if self.held is not None and self.held.exists() and not self.source.exists():
            self.held.rename(self.source); self.held = None
        self.result["cleanup_errors"] = errors
        if errors or self.records() or any(os.path.ismount(p) for p in [self.mountpoint, self.other_mount]):
            raise AssertionError("Normal CLI cleanup incomplete; owned artifacts/journals preserved")
        require_idle(self.footprint())

    def run(self):
        try:
            self.case("quiet-native-window", lambda: require_idle(self.footprint()))
            self.case("create-two-members-and-descriptor", self.create)
            self.case("no-overwrite", lambda: self.guarded_rejection(self.create_args(), password=self.password))
            self.case("auto-without-descriptor-needs-membership", self.absent_descriptor_diagnostic)
            self.case("auto-descriptor-mount", self.mount)
            self.case("file-fsync-atomic-sqlite-wal", self.write_workload)
            self.case("active-members-and-hardlink-aliases-refused", self.active_aliases)
            self.case("busy-close-preserves-array-and-journal", self.busy_session)
            self.case("normal-close", self.close)
            self.case("wrong-password-no-resources-or-byte-mutation", lambda: self.guarded_rejection(
                self.command("mount", mountpoint=self.mountpoint), password=self.wrong))
            self.case("readonly-refused-without-resources", lambda: self.guarded_rejection(
                self.command("mount", mountpoint=self.mountpoint, readonly=True), password=self.password))
            self.case("explicit-peer-mount", lambda: self.mount(kind="raid1c", peer=self.peer))
            self.case("explicit-peer-persistence", self.verify)
            self.case("explicit-peer-close", self.close)
            self.case("auto-reopen", self.mount)
            self.case("auto-reopen-persistence", self.verify)
            self.case("auto-reclose", self.close)
            self.case("missing-primary-explicit-survivor-and-offline-return", self.degraded_survivor)
            self.result["status"] = "passed"
        except Exception as error:
            self.result.update(status="failed", diagnostic=str(error))
        finally:
            if self.options.preserve_failure and self.result["status"] == "failed":
                self.result["cleanup_policy"] = "preserved-on-failure"
            else:
                try:
                    self.case("normal-cli-cleanup", self.normal_cleanup)
                except Exception as error:
                    self.result.update(status="failed", cleanup_diagnostic=str(error))
            self.result.update(finished=time.time(), retained=True,
                               mount_present=any(os.path.ismount(p) for p in [self.mountpoint, self.other_mount]))
            self.result["duration_seconds"] = round(self.result["finished"] - self.result["started"], 3)
            self.result["counts"] = {status: sum(case["status"] == status for case in self.result["cases"])
                                     for status in ["passed", "failed", "skipped"]}
            self.save()
        return self.result


USER_WORKLOAD = r'''import json, os, sqlite3, stat, sys
from pathlib import Path
action, stage, mounted, expected, uid, gid = sys.argv[1:]
uid, gid = int(uid), int(gid)
assert os.geteuid() == uid and uid != 0, "workload must run as the invoking normal user"
root = Path(mounted)
info = root.stat()
assert (info.st_uid, info.st_gid, stat.S_IMODE(info.st_mode)) == (uid, gid, 0o700)
payload = (stage + "\n").encode() + Path(expected).read_bytes()
file = root / "user-fsync.bin"
db = root / "user-profile.sqlite"
if action == "write":
    os.umask(0o077)
    with file.open("wb") as stream:
        stream.write(payload); stream.flush(); os.fsync(stream.fileno())
    connection = sqlite3.connect(db)
    try:
        assert connection.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
        connection.execute("PRAGMA synchronous=FULL")
        connection.execute("CREATE TABLE IF NOT EXISTS profile(key TEXT PRIMARY KEY, value BLOB)")
        connection.execute("INSERT OR REPLACE INTO profile VALUES (?,?)", ("latest", payload))
        connection.commit()
        reader = sqlite3.connect(db)
        try: assert reader.execute("SELECT value FROM profile WHERE key='latest'").fetchone()[0] == payload
        finally: reader.close()
    finally: connection.close()
    descriptor = os.open(root, os.O_RDONLY)
    try: os.fsync(descriptor)
    finally: os.close(descriptor)
assert file.read_bytes() == payload
connection = sqlite3.connect(db)
try:
    assert connection.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
    assert connection.execute("SELECT value FROM profile WHERE key='latest'").fetchone()[0] == payload
finally: connection.close()
assert file.stat().st_uid == uid and db.stat().st_uid == uid
print(json.dumps({"status":"passed", "uid":os.geteuid(), "action":action, "stage":stage,
                  "filesystem_owner":info.st_uid, "filesystem_mode":oct(stat.S_IMODE(info.st_mode))}))
'''


class InvokingOwnerSuite(MirrorSuite):
    """Fresh invoking-user fixture; exercise the opposite degraded direction."""
    def __init__(self, options):
        import pwd
        super().__init__(options)
        try:
            self.owner = pwd.getpwnam(options.owner_user)
        except KeyError as error:
            raise ValueError("--owner-user account does not exist") from error
        if self.owner.pw_uid == 0:
            raise ValueError("--owner-user must identify a normal account")
        # Assign only this newly allocated traversal directory and fresh helper
        # files. Member/descriptor/inner-root ownership must come from Lurker.
        os.chown(self.root, self.owner.pw_uid, self.owner.pw_gid)
        if self.member_parent != self.root:
            os.chown(self.member_parent, self.owner.pw_uid, self.owner.pw_gid)
        self.expected = self.root / "expected user payload.bin"
        self.expected.write_bytes(self.payload); self.expected.chmod(0o600)
        os.chown(self.expected, self.owner.pw_uid, self.owner.pw_gid)
        self.workload = self.root / "normal-user-workload.py"
        self.workload.write_text(USER_WORKLOAD); self.workload.chmod(0o600)
        os.chown(self.workload, self.owner.pw_uid, self.owner.pw_gid)
        self.result.update(suite="openbsd-raid1c-invoking-owner", invoking_user=options.owner_user,
                           invoking_uid=self.owner.pw_uid, invoking_gid=self.owner.pw_gid,
                           normal_user_workload=str(self.workload))
        self.save()

    def create_args(self):
        return ["/usr/bin/env", f"SUDO_UID={self.owner.pw_uid}", f"SUDO_GID={self.owner.pw_gid}", *super().create_args()]

    def file_owners(self):
        import stat
        values = {}
        for path in [self.source, self.peer, self.descriptor]:
            info = path.stat()
            value = {"uid":info.st_uid, "gid":info.st_gid, "mode":oct(stat.S_IMODE(info.st_mode))}
            if value != {"uid":self.owner.pw_uid, "gid":self.owner.pw_gid, "mode":"0o600"}:
                raise AssertionError(f"Lurker did not publish invoking-user-owned private storage: {path}")
            values[path.name] = value
        self.result["published_ownership"] = values; self.save()

    def user(self, action, stage):
        import shlex
        command = shlex.join(["/usr/local/bin/python3", str(self.workload), action, stage,
                              str(self.mountpoint), str(self.expected), str(self.owner.pw_uid), str(self.owner.pw_gid)])
        argv = ["/usr/bin/su", "-m", self.options.owner_user, "-c", command]
        process = subprocess.run(argv, stdin=subprocess.DEVNULL, capture_output=True, timeout=45)
        name = self.result["cases"][-1]["name"]
        records = self.result["cases"][-1].setdefault("normal_user_commands", [])
        log = self.logs / (name + f"-user-{len(records)}.log")
        log.write_bytes(process.stdout + process.stderr); log.chmod(0o600)
        if process.returncode:
            raise AssertionError(f"Normal-user {action}/{stage} failed exit{process.returncode}; {log}")
        value = json.loads(process.stdout)
        if value.get("uid") != self.owner.pw_uid or value.get("status") != "passed":
            raise AssertionError("Workload did not prove execution as the invoking normal user")
        records.append({"argv": argv, "log": str(log), "proof": value})

    def degraded_primary(self):
        held = self.root / "held missing secondary.img"
        self.peer.rename(held)
        # Primary0 survives, so the missing secondary1 can later be identified
        # as offline without the secondary-only role conflict tested separately.
        self.mount(kind="raid1c", degraded=True)
        self.user("write", "degraded-primary-newest")
        self.user("read", "degraded-primary-newest")
        self.degraded_status("primary-online-secondary-missing"); self.close()
        held.rename(self.peer)
        self.mount(kind="raid1c", peer=self.peer, degraded=True)
        self.user("read", "degraded-primary-newest")
        self.degraded_status("stale-secondary-returned-offline")
        self.close(); self.file_owners()
        self.result["native_member_headers"] = native_headers([self.source, self.peer]); self.save()
        self.result["limitations"].append("Returning secondary stays offline; newest invoking-user WAL/data must come from surviving primary. No rebuild attempted.")

    def run(self):
        browser = None
        try:
            if getattr(self.options, "firefox", None):
                self.options.firefox_account = self.owner
                browser = BASE.BrowserProfile(self)
                self.result["firefox"] = {"path": str(self.options.firefox),
                                          "sha256": BASE.file_hash(self.options.firefox),
                                          "uid": browser.uid}
            for name, function in [
                ("quiet-native-window", lambda: require_idle(self.footprint())),
                ("synthetic-sudo-create", self.create), ("published-uid-gid-and-0600", self.file_owners),
                ("root-mount-user-filesystem", self.mount),
                ("normal-user-write-fsync-and-wal", lambda: self.user("write", "initial")),
                ("normal-close", self.close), ("root-reopen-existing-ownership", self.mount),
                ("normal-user-read-reopened-wal", lambda: self.user("read", "initial")),
                ("normal-reclose", self.close),
                ("primary-survives-user-writes-secondary-return-offline", self.degraded_primary),
            ]:
                self.case(name, function)
                if browser and name == "normal-user-write-fsync-and-wal":
                    self.case("firefox-profile-write", lambda: browser.exercise("write"))
                if browser and name == "normal-user-read-reopened-wal":
                    self.case("firefox-profile-read", lambda: browser.exercise("read"))
            self.result["status"] = "passed"
        except Exception as error:
            self.result.update(status="failed", diagnostic=str(error))
        finally:
            if browser:
                browser.close()
            if self.options.preserve_failure and self.result["status"] == "failed":
                self.result["cleanup_policy"] = "preserved-on-failure"
            else:
                try: self.case("normal-cli-cleanup", self.normal_cleanup)
                except Exception as error: self.result.update(status="failed", cleanup_diagnostic=str(error))
            self.result.update(finished=time.time(), retained=True,
                               mount_present=any(os.path.ismount(p) for p in [self.mountpoint, self.other_mount]))
            self.result["duration_seconds"] = round(self.result["finished"] - self.result["started"], 3)
            self.result["counts"] = {status: sum(case["status"] == status for case in self.result["cases"])
                                     for status in ["passed", "failed", "skipped"]}
            self.save()
        return self.result


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--commit", required=True, help="Exact full source Git SHA of the supplied CLI artifact")
    parser.add_argument("--root", type=Path, default=Path("/tmp"))
    parser.add_argument("--size-gb", default="0.125", help="Size per member, at least 0.0625 GiB")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--long-parent", action="store_true", help="Put encrypted members and descriptor beneath a120-character directory; keep native mountpoints short")
    parser.add_argument("--owner-user", help="Use a separate fresh synthetic-sudo invoking-user proof instead of the root matrix")
    parser.add_argument("--firefox", type=Path, help="Exercise a fresh Firefox profile as the invoking user")
    parser.add_argument("--preserve-failure", action="store_true", help="Stop before cleanup on failure; preserve owned storage/journal for diagnosis")
    parser.add_argument("--full-storage", action="store_true", help="Run the bounded ENOSPC and recovery workload on a new mirror")
    options = parser.parse_args(argv)
    if platform.system() != "OpenBSD" or os.geteuid() != 0:
        parser.error("Run as root on the disposable OpenBSD guest; no storage tests on endeavour")
    if not re.fullmatch(r"[0-9a-f]{40}", options.commit) or options.timeout <= 0:
        parser.error("Use a full lowercase source SHA and positive timeout")
    if options.firefox:
        if not options.owner_user:
            parser.error("--firefox requires --owner-user")
        options.firefox = options.firefox.resolve()
        if not options.firefox.is_file() or not os.access(options.firefox, os.X_OK):
            parser.error("--firefox requires an executable Firefox")
    options.root = options.root.resolve(strict=True)
    if not options.root.is_dir() or not options.root.is_relative_to(Path("/tmp")):
        parser.error("--root must be an existing directory beneath /tmp")
    options.binary = options.binary.resolve(strict=True)
    private_binary(options.binary)
    options.backend = "raid1c"; options.cipher = "aes"; options.auto_detect = True
    options.expect_readonly_unsupported = True; options.skip_readonly = None; options.unprivileged = False
    BASE.validate_full_storage(parser, options)
    return options


def main(argv=None):
    try:
        options = parse_args(argv)
        for name in ["LURKER_BACKEND_DIR", "LURKER_USERSPACE_DIR", "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES", "SUDO_UID", "SUDO_GID", "PKEXEC_UID", "DOAS_USER"]:
            os.environ.pop(name, None)
        suite = InvokingOwnerSuite(options) if options.owner_user else MirrorSuite(options)
        result = suite.run_full_storage() if options.full_storage else suite.run()
        print(json.dumps({"schema": 1, "suite": result["suite"], "status": result["status"],
                          "source_commit": result["source_commit"], "counts": result["counts"],
                          "duration_seconds": result["duration_seconds"], "result": str(suite.root / "result.json"),
                          "diagnostic": result.get("diagnostic"), "cleanup_diagnostic": result.get("cleanup_diagnostic")}))
        return 0 if result["status"] == "passed" else 1
    except (OSError, ValueError) as error:
        print(json.dumps({"schema": 1, "status": "blocked", "diagnostic": str(error)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
