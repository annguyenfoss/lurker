#!/usr/bin/env python3
"""Small native CLI filesystem payload test; no SQLite, GUI or fault workloads.

Uses the established conformance terminal prompt handling. Every invocation
allocates a fresh private fixture and retains receipt/logs, including failures.
"""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import platform
import pty
import re
import secrets
import select
import signal
import tempfile
import termios
import time

DIRECTORIES = {"gocryptfs", "cryfs", "cryptomator", "pefs"}
PROMPT = re.compile(
    rb"(?im)(?:^|(?<=\r))[ \t]*(?:(?P<authorization>\[sudo\] |sudo )password"
    rb"|(?:(?:enter|re-enter|confirm|verify|existing|new|confirm new) )?(?:passphrase|password))"
    rb"(?: for [^\r\n]{1,400})?:[ \t]*(?=$|\r)"
)
MAX_TRANSCRIPT = 2 * 1024 * 1024
ECHO_WAIT_TIMEOUT = 5.0
ECHO_STABLE_SECONDS = 0.02
APPEND = b"\nAppended after a normal encrypted-container reopen.\n"
SECOND = b"Created during the second mount.\n"


class PayloadWorkerError(RuntimeError):
    def __init__(self, message, pid, remaining=False):
        super().__init__(message)
        self.worker = {"pid": pid, "remaining": remaining}


def stop_payload_worker(pid):
    """Signal only our unreaped child, without blocking on a stuck kernel I/O."""
    for sig in (signal.SIGTERM, signal.SIGKILL):
        child, _ = os.waitpid(pid, os.WNOHANG)
        if child:
            return True
        os.kill(pid, sig)
        deadline = time.monotonic() + 1
        while time.monotonic() < deadline:
            child, _ = os.waitpid(pid, os.WNOHANG)
            if child:
                return True
            time.sleep(0.02)
    return False


def bounded_payload_call(function, timeout):
    """Keep filesystem syscalls in a child so a broken driver cannot stall the round."""
    reader, writer = os.pipe()
    pid = os.fork()
    if pid == 0:
        os.close(reader)
        try:
            try:
                result = {"ok": True, "value": function()}
            except BaseException as error:
                result = {"ok": False, "error": str(error)[:2000]}
            data = json.dumps(result).encode()
            while data:
                data = data[os.write(writer, data):]
        finally:
            os.close(writer)
            os._exit(0)
    os.close(writer)
    finished = False
    data = bytearray()
    try:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if select.select([reader], [], [], 0.05)[0]:
                block = os.read(reader, 65536)
                data.extend(block)
                if len(data) > 65536:
                    raise PayloadWorkerError("Payload worker exceeded its result limit", pid)
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                finished = True
                while select.select([reader], [], [], 0)[0]:
                    block = os.read(reader, 65536)
                    if not block:
                        break
                    data.extend(block)
                if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    raise PayloadWorkerError("Payload worker exited unexpectedly", pid)
                try:
                    result = json.loads(data)
                except (ValueError, UnicodeDecodeError) as error:
                    raise PayloadWorkerError("Payload worker returned no valid result", pid) from error
                if not result["ok"]:
                    raise PayloadWorkerError(result["error"], pid)
                return result["value"]
        raise PayloadWorkerError(
            "Payload I/O timed out; retain this fixture and its native storage resources",
            pid)
    except BaseException as error:
        if not finished:
            # Never wait synchronously for an uninterruptible filesystem syscall.
            # Keep the owned PID in the receipt if it cannot be reaped.
            reaped = stop_payload_worker(pid)
            if isinstance(error, PayloadWorkerError):
                error.worker["remaining"] = not reaped
        raise
    finally:
        os.close(reader)

def stop_owned_child(pid):
    """Reap or stop our unreaped child; its PID cannot be reused meanwhile."""
    child, status = os.waitpid(pid, os.WNOHANG)
    if child:
        return status
    os.kill(pid, signal.SIGTERM)
    for _ in range(50):
        child, status = os.waitpid(pid, os.WNOHANG)
        if child:
            return status
        time.sleep(0.1)
    os.kill(pid, signal.SIGKILL)
    return os.waitpid(pid, 0)[1]

def atomic_json(path, value):
    temp = path.with_name(path.name + ".tmp")
    temp.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")
    os.chmod(temp, 0o600)
    os.replace(temp, path)

def file_hash(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()

def redact(data, secrets):
    for secret in secrets:
        if secret:
            data = data.replace(secret.encode(), b"[REDACTED]")
    return data.decode("utf-8", "replace")

def terminal(args, password, timeout, log):
    """Drive real CLI/native password prompts over its own controlling TTY.

    Only the process we forked is signalled on timeout. Its storage is retained
    for investigation; killing a CLI is never treated as successful cleanup.
    """
    pid, master = pty.fork()
    if pid == 0:
        try:
            os.execv(args[0], args)
        except OSError as error:
            try:
                os.write(2, f"Cannot execute CLI: {error.strerror}\n".encode())
            finally:
                os._exit(127)
    transcript = bytearray()
    consumed = 0
    prompts = 0
    deadline = time.monotonic() + timeout
    finished = False
    timed_out = False
    eof = False
    status = None
    pending_prompt = None
    echo_off_since = None
    try:
        while time.monotonic() < deadline:
            ready = [] if eof else select.select([master], [], [], 0.01 if pending_prompt else 0.1)[0]
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError as error:
                    if error.errno != errno.EIO:
                        raise
                    chunk = b""
                if not chunk:
                    eof = True
                else:
                    transcript.extend(chunk)
                    if len(transcript) > MAX_TRANSCRIPT:
                        raise RuntimeError("CLI exceeded bounded transcript limit")
                    # sudo may prefix a native prompt with OSC 3008 session
                    # metadata. Match displayed text; retain raw, redacted logs.
                    visible = re.sub(rb"\x1b\].*?(?:\x07|\x1b\\)", b"", bytes(transcript), flags=re.S)
                    visible = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", visible)
                    match = None if pending_prompt else PROMPT.search(visible[consumed:])
                    if match:
                        # This suite never supplies administrator credentials.
                        if match.group("authorization"):
                            raise RuntimeError("Unexpected authorization prompt; run with the explicit required privilege route")
                        if password is None:
                            raise RuntimeError("Unexpected credential prompt")
                        consumed += match.end()
                        pending_prompt = time.monotonic()
                        echo_off_since = None
            child, status = os.waitpid(pid, os.WNOHANG)
            if child:
                finished = True
                break
            if pending_prompt is not None:
                # cryptsetup may print before tcsetattr(TCSAFLUSH) disables
                # echo and flushes queued input. Prompt text is not readiness.
                now = time.monotonic()
                echo_off = not (termios.tcgetattr(master)[3] & termios.ECHO)
                if echo_off:
                    if echo_off_since is None:
                        echo_off_since = now
                    if now - echo_off_since >= ECHO_STABLE_SECONDS:
                        prompts += 1
                        if prompts > 8:
                            raise RuntimeError("CLI exceeded password prompt limit")
                        os.write(master, password.encode() + b"\n")
                        pending_prompt = None
                        echo_off_since = None
                else:
                    echo_off_since = None
                if pending_prompt is not None and now - pending_prompt >= ECHO_WAIT_TIMEOUT:
                    raise RuntimeError("Credential prompt never disabled terminal echo; no credential sent")
            if eof:
                time.sleep(0.02)
        if not finished:
            timed_out = True
            status = stop_owned_child(pid)
            finished = True
        # Drain already available output after the child has exited.
        while not eof and select.select([master], [], [], 0)[0]:
            try:
                chunk = os.read(master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                break
            if not chunk:
                break
            transcript.extend(chunk)
            if len(transcript) > MAX_TRANSCRIPT:
                raise RuntimeError("CLI exceeded bounded transcript limit")
        if password and password.encode() in transcript:
            raise RuntimeError("CLI exposed a credential in its terminal output")
        return {"exit_code": os.waitstatus_to_exitcode(status), "prompts": prompts,
                "timed_out": timed_out}
    finally:
        if not finished:
            # Direct owned child only, never an inferred PID/session or VM.
            try:
                stop_owned_child(pid)
            except (ProcessLookupError, ChildProcessError):
                pass
        os.close(master)
        log.write_text(redact(bytes(transcript), [password]))
        os.chmod(log, 0o600)


def sample_pdf():
    """A small valid one-page PDF, avoiding a generation dependency."""
    stream = b"BT /F1 12 Tf 72 720 Td (Lurker filesystem persistence test) Tj ET\n"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        b"<< /Length " + str(len(stream)).encode() + b" >>\nstream\n" + stream + b"endstream",
    ]
    data = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for number, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(str(number).encode() + b" 0 obj\n" + obj + b"\nendobj\n")
    xref = len(data)
    data.extend(b"xref\n0 6\n0000000000 65535 f \n")
    for offset in offsets[1:]:
        data.extend(f"{offset:010d} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(data)


def payload_write(path, data, append=False):
    with path.open("ab" if append else "xb") as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())


def load_payload(payload_dir=None, pdf=None):
    if payload_dir is not None:
        if pdf is not None:
            raise ValueError("Choose --payload-dir or --pdf, not both")
        expected = {}
        for name in ("bitcoin.pdf", "ethereum.pdf", "transformer.pdf", "page.txt"):
            path = payload_dir / name
            if path.is_symlink() or not path.is_file():
                raise ValueError("Missing regular payload file: " + str(path))
            contents = path.read_bytes()
            if not contents or (name.endswith(".pdf") and not contents.startswith(b"%PDF-")):
                raise ValueError("Invalid payload file: " + str(path))
            expected[name] = contents
        return expected
    contents = pdf.read_bytes() if pdf else sample_pdf()
    if not contents.startswith(b"%PDF-"):
        raise ValueError("Payload must be a PDF")
    return {"paper.pdf": contents,
            "page.txt": ("Lurker filesystem payload.\nUnicode: Việt Nam — 日本語.\n" * 32).encode()}


def verify_payload(mountpoint, expected):
    if not os.path.ismount(mountpoint):
        raise RuntimeError("CLI did not establish an OS mount; refusing underlay access")
    result = {}
    for name, contents in expected.items():
        path = mountpoint / "Lurker payload" / name
        if path.is_symlink() or path.read_bytes() != contents:
            raise RuntimeError("Payload mismatch: " + name)
        result[name] = hashlib.sha256(contents).hexdigest()
    return result


def linux_mount_record(text, path):
    target = str(path)
    matches = []
    for line in text.splitlines():
        left, separator, right = line.partition(" - ")
        before, after = left.split(), right.split()
        if not separator or len(before) < 6 or len(after) < 2:
            continue
        point = re.sub(r"\\([0-7]{3})", lambda m: chr(int(m[1], 8)), before[4])
        if target != point and not target.startswith(point.rstrip("/") + "/"):
            continue
        matches.append({"mountpoint": point, "filesystem": after[0],
                        "source": re.sub(r"\\([0-7]{3})", lambda m: chr(int(m[1], 8)), after[1]),
                        "options": before[5].split(","), "mount_id": before[0]})
    if not matches:
        raise RuntimeError("Native Linux mount identity is missing")
    longest = max(len(row["mountpoint"]) for row in matches)
    matches = [row for row in matches if len(row["mountpoint"]) == longest]
    if len(matches) != 1:
        raise RuntimeError("Native Linux mount identity is ambiguous")
    return matches[0]


def check_linux_filesystem(record, requested):
    if "rw" not in record["options"]:
        raise RuntimeError("Native mount is not read/write")
    if not requested:
        return "recorded"
    expected = {
        "ext4": {"ext4"}, "btrfs": {"btrfs"}, "xfs": {"xfs"}, "apfs": {"apfs"},
        "hammer2": {"hammer2"}, "exfat": {"exfat", "fuseblk", "fuse.exfat"},
        "zfs": {"zfs"}, "fat32": {"vfat"}, "ufs1": {"ufs"},
        "ufs2": {"fuse.rufs", "fuse.fuse-ufs", "fuse", "fuseblk"},
        "ntfs": {"ntfs3", "ntfs", "fuseblk", "fuse.ntfs-3g"},
    }
    if record["filesystem"] not in expected.get(requested, set()):
        raise RuntimeError(f"Requested {requested} but native mount reports {record['filesystem']}")
    # Generic FUSE labels and UFS family names need the driver's/superblock's
    # independent identity evidence; mount type alone cannot prove that format.
    if record["filesystem"] in {"fuse", "fuseblk", "ufs"} or requested == "ufs2":
        return "driver-or-superblock-proof-required"
    return "native-type-verified"


def payload_phase(mountpoint, expected, phase, filesystem=None, backend=None, source=None):
    if not os.path.ismount(mountpoint):
        raise RuntimeError("Mount missing; refusing payload write to underlay")
    result = {}
    if platform.system() == "Linux":
        mountinfo = Path("/proc/self/mountinfo").read_text()
        mounted = linux_mount_record(mountinfo, mountpoint)
        if mounted["mountpoint"] != str(mountpoint):
            raise RuntimeError("Native mount table does not contain the plaintext mountpoint")
        result[phase + "_mount"] = mounted
        identity = mounted
        if backend in DIRECTORIES:
            if source is None:
                raise RuntimeError("Directory filesystem verification needs its ciphertext source")
            identity = linux_mount_record(mountinfo, source)
            result[phase + "_backing_mount"] = identity
        result[phase + "_filesystem_identity"] = check_linux_filesystem(identity, filesystem)
    if phase == "write":
        metadata = mountpoint.stat()
        result["initial_root_metadata"] = {
            "uid": metadata.st_uid, "gid": metadata.st_gid,
            "mode": oct(metadata.st_mode & 0o7777),
        }
        payload_dir = mountpoint / "Lurker payload"
        payload_dir.mkdir(mode=0o700)
        for name, contents in expected.items():
            payload_write(payload_dir / name, contents)
    else:
        verify_payload(mountpoint, expected)
        if phase == "append":
            payload_write(mountpoint / "Lurker payload/page.txt", APPEND, append=True)
            expected["page.txt"] += APPEND
            expected["second.txt"] = SECOND
            payload_write(mountpoint / "Lurker payload/second.txt", SECOND)
    result[phase + "_sha256"] = verify_payload(mountpoint, expected)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--backend", required=True)
    parser.add_argument("--cipher", choices=("aes", "serpent", "twofish"))
    parser.add_argument("--credential", choices=("password", "keyfile"), default="password")
    parser.add_argument("--auto-detect", action="store_true", help="Reopen with --type auto")
    parser.add_argument("--filesystem", help="Create selection; backing-store label only for directory engines")
    parser.add_argument("--root", type=Path, default=Path(tempfile.gettempdir()))
    parser.add_argument("--mount-root", type=Path,
                        help="Separate plaintext mount parent for directory backing-store tests")
    parser.add_argument("--size-gb", default="0.5")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--pdf", type=Path)
    parser.add_argument("--payload-dir", type=Path,
                        help="Common bitcoin.pdf, ethereum.pdf, transformer.pdf and page.txt inputs")
    parser.add_argument("--unprivileged", action="store_true")
    parser.add_argument("--timeout", type=int, default=240)
    args = parser.parse_args()
    if args.cipher and args.backend in DIRECTORIES:
        parser.error("Directory backends use a fixed profile, without --cipher")
    if args.credential == "keyfile" and args.backend not in {"luks", "luks1", "veracrypt", "geli", "pefs"}:
        parser.error("This backend has no binary-keyfile credential route")
    try:
        expected = load_payload(args.payload_dir, args.pdf)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    binary = args.binary.resolve(strict=True)
    parent = args.root.resolve(strict=True)
    directory = Path(tempfile.mkdtemp(prefix="lurker-fs-cli-", dir=parent))
    directory.chmod(0o700)
    if args.mount_root:
        mountpoint = Path(tempfile.mkdtemp(
            prefix="lurker-fs-mount-", dir=args.mount_root.resolve(strict=True)))
        mountpoint.chmod(0o700)
    else:
        mountpoint = directory / "mount"
        mountpoint.mkdir(mode=0o700)
    source = directory / ("encrypted" if args.backend in DIRECTORIES else "vault.img")
    peer = directory / "peer.img"
    # Meet ordinary host password policies without changing their settings.
    password = "Lurker9!" + secrets.token_urlsafe(24)
    keyfile = None
    if args.credential == "keyfile":
        password = None
        keyfile = directory / "binary key, Việt Nam.bin"
        descriptor = os.open(keyfile, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(b"\x00\xff\n\r" + secrets.token_bytes(252))
            stream.flush()
            os.fsync(stream.fileno())
    result = {"schema": 1, "os": platform.system(), "release": platform.release(),
              "machine": platform.machine(), "commit": args.commit,
              "binary": str(binary), "binary_sha256": file_hash(binary),
              "backend": args.backend, "filesystem": args.filesystem,
              "cipher": args.cipher, "credential": args.credential,
              "auto_detect": args.auto_detect,
              "keyfile": str(keyfile) if keyfile else None,
              "keyfile_sha256": file_hash(keyfile) if keyfile else None,
              "unprivileged": args.unprivileged,
              "payload_dir": str(args.payload_dir.resolve()) if args.payload_dir else None,
              "input_sha256": {name: hashlib.sha256(data).hexdigest()
                               for name, data in expected.items()},
              "source": str(source), "mountpoint": str(mountpoint),
              "fixture": str(directory), "cases": [], "status": "running"}
    receipt = directory / "result.json"
    atomic_json(receipt, result)
    mounted = False

    def mount_present():
        try:
            return bounded_payload_call(lambda: os.path.ismount(mountpoint), min(args.timeout, 5))
        except PayloadWorkerError as error:
            result["mount_probe_error"] = str(error)
            result.setdefault("payload_workers", []).append(error.worker)
            # Unknown mount state must retain recovery resources, never claim cleanup.
            return True

    def cli(action, operands):
        backend = "auto" if args.auto_detect and action == "mount" else args.backend
        command = [str(binary), action, "--type", backend, "--color=never"]
        if action == "create" and args.cipher:
            command.extend(["--cipher", args.cipher])
        if action in {"create", "mount"} and keyfile:
            command.extend(["--key-file", str(keyfile)])
        if args.unprivileged:
            command.append("--unprivileged")
        command.extend(map(str, operands))
        log = directory / f"{len(result['cases']):02d}-{action}.log"
        status = terminal(command, password, args.timeout, log)
        result["cases"].append({"action": action, "log": str(log), **status})
        atomic_json(receipt, result)
        if status["exit_code"] != 0 or status["timed_out"]:
            raise RuntimeError(f"CLI {action} failed; see {log}")

    try:
        operands = []
        if args.filesystem and args.backend not in DIRECTORIES:
            operands.extend(["--filesystem", args.filesystem])
        if args.backend == "raid1c":
            operands.extend(["--member", peer])
        operands.append(source)
        if args.backend not in DIRECTORIES:
            operands.append(args.size_gb)
        cli("create", operands)
        for phase in ("write", "append", "verify"):
            cli("mount", [source, mountpoint])
            mounted = True
            result["payload_phase"] = phase
            atomic_json(receipt, result)
            result.update(bounded_payload_call(
                lambda: payload_phase(mountpoint, expected, phase, args.filesystem,
                                      args.backend, source), args.timeout))
            if phase == "append":
                expected["page.txt"] += APPEND
                expected["second.txt"] = SECOND
            cli("unmount", [mountpoint])
            mounted = False
            if mount_present():
                raise RuntimeError("CLI close left the payload mounted")
        result["status"] = "passed"
    except Exception as error:
        result["status"] = "failed"
        result["error"] = str(error)
        if isinstance(error, PayloadWorkerError):
            result.setdefault("payload_workers", []).append(error.worker)
    finally:
        if mounted or mount_present():
            try:
                cli("unmount", [mountpoint])
            except Exception as error:
                result["cleanup_error"] = str(error)
        result["mount_remaining"] = mount_present()
        result["finished"] = time.time()
        atomic_json(receipt, result)
    print(json.dumps({"status": result["status"], "receipt": str(receipt),
                      "cases": len(result["cases"]), "mount_remaining": result["mount_remaining"]}))
    return 0 if result["status"] == "passed" and not result["mount_remaining"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
