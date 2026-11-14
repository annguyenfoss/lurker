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
        self.peer = self.root / "peer 密碼 with spaces.img"
        self.descriptor = Path(str(self.source) + ".raid1c.json")
        self.held = None
        self.result.update(suite="openbsd-raid1c-cli", profile="two members; native AES-XTS/bcrypt_pbkdf; FFS2",
                           size_gb_per_member=options.size_gb,
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
        # Reattach both files only under explicit degraded recovery. The stale
        # returning primary stays offline; neither this suite nor Lurker rebuilds it.
        self.mount(kind="raid1c", peer=self.peer, degraded=True)
        self.verify(); self.degraded_status("returning-primary-stays-offline"); self.close()

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


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--commit", required=True, help="Exact full source Git SHA of the supplied CLI artifact")
    parser.add_argument("--root", type=Path, default=Path("/tmp"))
    parser.add_argument("--size-gb", default="0.125", help="Size per member, at least 0.0625 GiB")
    parser.add_argument("--timeout", type=float, default=180)
    options = parser.parse_args(argv)
    if platform.system() != "OpenBSD" or os.geteuid() != 0:
        parser.error("Run as root on the disposable OpenBSD guest; no storage tests on endeavour")
    if not re.fullmatch(r"[0-9a-f]{40}", options.commit) or options.timeout <= 0:
        parser.error("Use a full lowercase source SHA and positive timeout")
    options.root = options.root.resolve(strict=True)
    if not options.root.is_dir() or not options.root.is_relative_to(Path("/tmp")):
        parser.error("--root must be an existing directory beneath /tmp")
    options.binary = options.binary.resolve(strict=True)
    private_binary(options.binary)
    options.backend = "raid1c"; options.cipher = "aes"; options.auto_detect = True
    options.expect_readonly_unsupported = True; options.skip_readonly = None; options.unprivileged = False
    return options


def main(argv=None):
    try:
        options = parse_args(argv)
        for name in ["LURKER_BACKEND_DIR", "LURKER_USERSPACE_DIR", "LD_PRELOAD", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "DYLD_INSERT_LIBRARIES"]:
            os.environ.pop(name, None)
        suite = MirrorSuite(options)
        result = suite.run()
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
