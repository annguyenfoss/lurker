#!/usr/bin/env python3
"""Exercise a disposable namespace beneath an identity-checked Linux mount.

Run as the intended payload caller. Keep the receipt outside the tested mount.
On failure, retain the unique test directory and the enclosing fixture. A timed
out worker may still hold the mount; its existence is never a cleanup success.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import stat
import sys
import tempfile
import time


PAYLOAD = b"Lurker mounted namespace\x00\n" + bytes(range(256)) * 128


def unescape(value):
    for encoded, decoded in ((r"\040", " "), (r"\011", "\t"),
                             (r"\012", "\n"), (r"\134", "\\")):
        value = value.replace(encoded, decoded)
    return value


def mount_record(contents, mountpoint):
    records = []
    for line in contents.splitlines():
        left, right = line.split(" - ", 1)
        fields, filesystem = left.split(), right.split()
        if unescape(fields[4]) == str(mountpoint):
            records.append({"mount_id": fields[0], "device": fields[2],
                            "root": unescape(fields[3]), "mountpoint": unescape(fields[4]),
                            "options": fields[5].split(","), "fstype": filesystem[0],
                            "source": unescape(filesystem[1]),
                            "superblock_options": filesystem[2].split(",")})
    if len(records) != 1:
        raise RuntimeError("Expected exactly one native mount at the supplied root")
    return records[0]


def validate_record(record, mount_id, source, fstype):
    if (record["mount_id"], record["source"], record["fstype"]) != (mount_id, source, fstype):
        raise RuntimeError("Native mount ID/source/filesystem differs from the expected mount")
    if any("ro" in record[key] or "rw" not in record[key]
           for key in ("options", "superblock_options")):
        raise RuntimeError("Native mount is not read/write in both option fields")


def identity(fd):
    value = os.fstat(fd)
    return {"device": value.st_dev, "inode": value.st_ino,
            "uid": value.st_uid, "gid": value.st_gid,
            "mode": oct(stat.S_IMODE(value.st_mode))}


def save(path, record):
    descriptor, temporary = tempfile.mkstemp(prefix=path.name + ".", dir=path.parent)
    try:
        with os.fdopen(descriptor, "w") as stream:
            json.dump(record, stream, indent=2)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


class MountedNamespace:
    def __init__(self, args, receipt):
        if os.geteuid() != args.expect_euid:
            raise RuntimeError("Actual effective UID differs from the intended payload caller")
        self.args, self.receipt = args, receipt
        self.path = args.mountpoint.resolve(strict=True)
        if self.path == self.path.parent:
            raise RuntimeError("Refusing the operating-system root mount")
        self.fd = os.open(self.path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
        self.initial = identity(self.fd)
        self.guard()
        if args.receipt.resolve().is_relative_to(self.path):
            raise RuntimeError("Receipt resolves inside the tested mount")

    def guard(self):
        record = mount_record(Path("/proc/self/mountinfo").read_text(), self.path)
        validate_record(record, self.args.expect_mount_id, self.args.expect_source,
                        self.args.expect_fstype)
        fdinfo = dict(line.split(":", 1) for line in
                      Path(f"/proc/self/fdinfo/{self.fd}").read_text().splitlines())
        if fdinfo["mnt_id"].strip() != record["mount_id"]:
            raise RuntimeError("Held directory descriptor belongs to a different mount")
        current = os.stat(self.path, follow_symlinks=False)
        held = os.fstat(self.fd)
        if (current.st_dev, current.st_ino) != (held.st_dev, held.st_ino):
            raise RuntimeError("Mounted path no longer resolves to the held root")
        if identity(self.fd) != self.initial:
            raise RuntimeError("Held mount-root identity or metadata changed")
        if os.geteuid() != self.args.expect_euid:
            raise RuntimeError("Payload caller changed during namespace operations")
        return record

    def action(self, name, operation):
        entry = {"action": name, "status": "running", "started": time.time(),
                 "actual_euid": os.geteuid(), "mount_before": self.guard()}
        self.receipt["actions"].append(entry)
        save(self.args.receipt, self.receipt)
        try:
            entry["observation"] = operation()
            entry["mount_after"] = self.guard()
            entry["status"] = "passed"
        except BaseException as error:
            entry.update(status="failed", error=repr(error))
            raise
        finally:
            entry["finished"] = time.time()
            save(self.args.receipt, self.receipt)

    @staticmethod
    def file_hash(fd, name):
        descriptor = os.open(name, os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        try:
            if not stat.S_ISREG(os.fstat(descriptor).st_mode):
                raise RuntimeError("Expected a regular namespace file")
            with os.fdopen(descriptor, "rb", closefd=False) as stream:
                return hashlib.file_digest(stream, "sha256").hexdigest()
        finally:
            os.close(descriptor)

    @staticmethod
    def absent(fd, name):
        try:
            os.stat(name, dir_fd=fd, follow_symlinks=False)
        except FileNotFoundError:
            return True
        raise RuntimeError("Renamed/deleted entry is still present")

    def write(self, fd, name, contents):
        descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                             0o600, dir_fd=fd)
        try:
            with os.fdopen(descriptor, "wb", closefd=False) as stream:
                stream.write(contents)
                stream.flush()
                os.fsync(descriptor)
            metadata = identity(descriptor)
        finally:
            os.close(descriptor)
        os.fsync(fd)
        wanted = hashlib.sha256(contents).hexdigest()
        if self.file_hash(fd, name) != wanted:
            raise RuntimeError("Created namespace file differs from its written bytes")
        return {"sha256": wanted, "metadata": metadata, "file_and_parent_fsync": True}

    def protected(self):
        result = {}

        def visit(fd, prefix):
            with os.scandir(fd) as entries:
                names = sorted(entry.name for entry in entries)
            for name in names:
                item = os.stat(name, dir_fd=fd, follow_symlinks=False)
                key = prefix + "/" + name
                if stat.S_ISDIR(item.st_mode):
                    child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                    try:
                        visit(child, key)
                    finally:
                        os.close(child)
                elif stat.S_ISREG(item.st_mode):
                    result[key] = {"sha256": self.file_hash(fd, name), "bytes": item.st_size,
                                   "uid": item.st_uid, "gid": item.st_gid,
                                   "mode": oct(stat.S_IMODE(item.st_mode))}
                else:
                    raise RuntimeError("Protected tree contains a link or special object")
                if len(result) > 10000:
                    raise RuntimeError("Protected tree exceeds the bounded fixture inventory")

        for name in self.args.protect_tree:
            if name in ("", ".", "..") or "/" in name:
                raise ValueError("Protected tree must be a single relative directory name")
            fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=self.fd)
            try:
                visit(fd, name)
            finally:
                os.close(fd)
        return result

    def run(self):
        before = self.protected()
        self.receipt.update(mount_root=self.initial, protected_before=before)
        name = "lurker-namespace-" + secrets.token_hex(12)
        self.receipt["test_directory"] = str(self.path / name)
        self.action("mkdir-and-sync-test-directory",
                    lambda: (os.mkdir(name, 0o700, dir_fd=self.fd), os.fsync(self.fd)))
        fd = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=self.fd)
        child = None
        try:
            self.action("create-write-fsync-readhash-file", lambda: self.write(fd, "before.bin", PAYLOAD))
            self.action("mkdir-and-sync-child", lambda: (os.mkdir("directory.before", 0o700, dir_fd=fd), os.fsync(fd)))
            child = os.open("directory.before", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            self.action("create-write-fsync-readhash-child", lambda: self.write(child, "child.bin", PAYLOAD[::-1]))

            def rename_file():
                os.rename("before.bin", "after.bin", src_dir_fd=fd, dst_dir_fd=fd)
                os.fsync(fd)
                self.absent(fd, "before.bin")
                digest = self.file_hash(fd, "after.bin")
                if digest != hashlib.sha256(PAYLOAD).hexdigest():
                    raise RuntimeError("File rename changed bytes")
                return {"sha256": digest, "old_name_absent": True, "parent_fsync": True}

            self.action("rename-file-and-readhash", rename_file)

            def rename_directory():
                os.rename("directory.before", "directory.after", src_dir_fd=fd, dst_dir_fd=fd)
                os.fsync(fd)
                self.absent(fd, "directory.before")
                moved = os.open("directory.after", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                try:
                    if identity(moved) != identity(child):
                        raise RuntimeError("Directory rename changed held directory identity")
                    digest = self.file_hash(moved, "child.bin")
                    if digest != hashlib.sha256(PAYLOAD[::-1]).hexdigest():
                        raise RuntimeError("Directory rename changed child bytes")
                finally:
                    os.close(moved)
                return {"sha256": digest, "old_name_absent": True, "parent_fsync": True}

            self.action("rename-directory-and-readhash-child", rename_directory)

            def cross_parent():
                os.rename("after.bin", "moved.bin", src_dir_fd=fd, dst_dir_fd=child)
                os.fsync(fd)
                os.fsync(child)
                self.absent(fd, "after.bin")
                digest = self.file_hash(child, "moved.bin")
                if digest != hashlib.sha256(PAYLOAD).hexdigest():
                    raise RuntimeError("Cross-parent rename changed bytes")
                return {"sha256": digest, "old_name_absent": True, "both_parents_fsync": True}

            self.action("cross-parent-rename-and-readhash", cross_parent)
            for entry in ("child.bin", "moved.bin"):
                def remove(entry=entry):
                    os.unlink(entry, dir_fd=child)
                    os.fsync(child)
                    return {"absent": self.absent(child, entry), "parent_fsync": True}
                self.action("unlink-" + entry, remove)
            os.close(child)
            child = None
            self.action("rmdir-and-sync-child", lambda: (os.rmdir("directory.after", dir_fd=fd), os.fsync(fd), self.absent(fd, "directory.after")))
            if os.listdir(fd):
                raise RuntimeError("Unexpected remnant in the owned namespace")
        finally:
            if child is not None:
                os.close(child)
            os.close(fd)
        self.action("rmdir-and-sync-test-directory", lambda: (os.rmdir(name, dir_fd=self.fd), os.fsync(self.fd), self.absent(self.fd, name)))
        after = self.protected()
        if before != after:
            raise RuntimeError("Protected owner payload or metadata changed")
        self.receipt.update(protected_after=after, protected_unchanged=bool(self.args.protect_tree),
                            test_directory_remaining=False, status="passed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mountpoint", type=Path)
    parser.add_argument("--expect-mount-id", required=True)
    parser.add_argument("--expect-source", required=True)
    parser.add_argument("--expect-fstype", required=True)
    parser.add_argument("--expect-euid", required=True, type=int)
    parser.add_argument("--receipt", required=True, type=Path)
    parser.add_argument("--protect-tree", action="append", default=[])
    parser.add_argument("--timeout", type=float, default=45)
    args = parser.parse_args()
    if not 0 < args.timeout <= 300:
        parser.error("Timeout must be in (0, 300] seconds")
    args.receipt = args.receipt.absolute()
    # Do not resolve/stat the tested filesystem in the supervising process.
    # The bounded worker performs the canonical-path and held-FD admission.
    if args.receipt.is_relative_to(args.mountpoint.absolute()):
        parser.error("Receipt must be outside the tested mount")
    os.umask(0o077)
    descriptor = os.open(args.receipt, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    os.close(descriptor)
    receipt = {"schema": 1, "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
               "actual_uid": os.getuid(), "actual_euid": os.geteuid(), "actual_gid": os.getgid(),
               "actual_egid": os.getegid(), "actual_groups": os.getgroups(), "status": "starting",
               "expected": {"mount_id": args.expect_mount_id, "source": args.expect_source,
                            "fstype": args.expect_fstype, "euid": args.expect_euid},
               "started": time.time(), "actions": []}
    save(args.receipt, receipt)
    pid = os.fork()
    if pid == 0:
        mounted = None
        try:
            receipt.update(worker_pid=os.getpid(), status="running")
            mounted = MountedNamespace(args, receipt)
            mounted.run()
        except BaseException as error:
            receipt.update(status="failed", error=repr(error),
                           preserve_unexpected_remnants=True)
        finally:
            if mounted is not None:
                os.close(mounted.fd)
            receipt["finished"] = time.time()
            save(args.receipt, receipt)
        os._exit(0 if receipt["status"] == "passed" else 1)
    deadline = time.monotonic() + args.timeout
    wait_status = None
    while time.monotonic() < deadline:
        waited, status = os.waitpid(pid, os.WNOHANG)
        if waited:
            wait_status = status
            break
        time.sleep(0.05)
    else:
        os.kill(pid, signal.SIGKILL)
        terminal = False
        stop_deadline = time.monotonic() + 1
        while time.monotonic() < stop_deadline:
            waited, status = os.waitpid(pid, os.WNOHANG)
            if waited:
                terminal = True
                wait_status = status
                break
            time.sleep(0.05)
        receipt = json.loads(args.receipt.read_text())
        receipt.update(status="timed-out", worker_pid=pid, worker_remaining=not terminal,
                       preserve_unexpected_remnants=True,
                       cleanup_prohibited_until_worker_and_mount_reviewed=True)
        save(args.receipt, receipt)
    receipt = json.loads(args.receipt.read_text())
    receipt["native_wait_status"] = wait_status
    clean_exit = (wait_status is not None and os.WIFEXITED(wait_status)
                  and os.WEXITSTATUS(wait_status) == 0)
    receipt["worker_exited_zero"] = clean_exit
    if wait_status is not None:
        receipt["worker_exit_code"] = os.waitstatus_to_exitcode(wait_status)
    if receipt["status"] == "passed" and not clean_exit:
        receipt.update(status="failed-worker-exit", preserve_unexpected_remnants=True,
                       error="Worker did not exit normally with status zero")
    save(args.receipt, receipt)
    print(json.dumps({"status": receipt["status"], "receipt": str(args.receipt),
                      "worker_pid": pid, "actions": len(receipt["actions"])}))
    return 0 if receipt["status"] == "passed" else 1


if __name__ == "__main__":
    sys.exit(main())
