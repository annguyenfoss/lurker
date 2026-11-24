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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--backend", required=True)
    parser.add_argument("--filesystem", help="Create selection; backing-store label only for directory engines")
    parser.add_argument("--root", type=Path, default=Path(tempfile.gettempdir()))
    parser.add_argument("--size-gb", default="0.5")
    parser.add_argument("--commit", required=True)
    parser.add_argument("--pdf", type=Path)
    parser.add_argument("--unprivileged", action="store_true")
    parser.add_argument("--timeout", type=int, default=240)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    parent = args.root.resolve(strict=True)
    directory = Path(tempfile.mkdtemp(prefix="lurker-fs-cli-", dir=parent))
    directory.chmod(0o700)
    mountpoint = directory / "mount"
    mountpoint.mkdir(mode=0o700)
    source = directory / ("encrypted" if args.backend in DIRECTORIES else "vault.img")
    peer = directory / "peer.img"
    # Meet ordinary host password policies without changing their settings.
    password = "Lurker9!" + secrets.token_urlsafe(24)
    pdf = args.pdf.read_bytes() if args.pdf else sample_pdf()
    if not pdf.startswith(b"%PDF-"):
        parser.error("Payload must be a PDF")
    expected = {"paper.pdf": pdf, "page.txt": ("Lurker filesystem payload.\nUnicode: Việt Nam — 日本語.\n" * 32).encode()}
    result = {"schema": 1, "os": platform.system(), "release": platform.release(),
              "machine": platform.machine(), "commit": args.commit,
              "binary": str(binary), "binary_sha256": file_hash(binary),
              "backend": args.backend, "filesystem": args.filesystem,
              "unprivileged": args.unprivileged,
              "source": str(source), "mountpoint": str(mountpoint),
              "fixture": str(directory), "cases": [], "status": "running"}
    receipt = directory / "result.json"
    atomic_json(receipt, result)
    mounted = False

    def cli(action, operands):
        command = [str(binary), action, "--type", args.backend, "--color=never"]
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
            if not os.path.ismount(mountpoint):
                raise RuntimeError("Mount missing; refusing payload write to underlay")
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
                    addition = b"\nAppended after a normal encrypted-container reopen.\n"
                    payload_write(mountpoint / "Lurker payload/page.txt", addition, append=True)
                    expected["page.txt"] += addition
                    expected["second.txt"] = b"Created during the second mount.\n"
                    payload_write(mountpoint / "Lurker payload/second.txt", expected["second.txt"])
            result[phase + "_sha256"] = verify_payload(mountpoint, expected)
            cli("unmount", [mountpoint])
            mounted = False
            if os.path.ismount(mountpoint):
                raise RuntimeError("CLI close left the payload mounted")
        result["status"] = "passed"
    except Exception as error:
        result["status"] = "failed"
        result["error"] = str(error)
    finally:
        if mounted or os.path.ismount(mountpoint):
            try:
                cli("unmount", [mountpoint])
            except Exception as error:
                result["cleanup_error"] = str(error)
        result["mount_remaining"] = os.path.ismount(mountpoint)
        result["finished"] = time.time()
        atomic_json(receipt, result)
    print(json.dumps({"status": result["status"], "receipt": str(receipt),
                      "cases": len(result["cases"]), "mount_remaining": result["mount_remaining"]}))
    return 0 if result["status"] == "passed" and not result["mount_remaining"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
