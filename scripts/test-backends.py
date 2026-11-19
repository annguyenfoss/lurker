#!/usr/bin/env python3
"""Native CLI conformance on newly allocated, private disposable fixtures.

No storage operation runs until --binary and --backend are explicitly supplied.
The runner never force-unmounts, detaches devices itself, or repairs storage.
"""
import argparse
import errno
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import pty
import re
import select
import signal
import sqlite3
import subprocess
import sys
import tempfile
import termios
import threading
import time

SCHEMA = 1
DIRECTORIES = {"gocryptfs", "cryfs", "cryptomator", "pefs"}
BACKENDS = {"luks", "luks1", "veracrypt", "apple", "geli", "cgd", "softraid", *DIRECTORIES}
PROMPT = re.compile(rb"(?i)(?:passphrase|password)[^\r\n:]{0,400}:[ \t]*")
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


class BrowserProfile:
    """Exercise a new Firefox profile against a local-only test origin."""

    def __init__(self, suite):
        self.suite = suite
        self.token = os.urandom(16).hex()
        received = self.received = []
        token = self.token

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path in {"/write", "/read"}:
                    initialize = f"localStorage.setItem('lurker-fixture', '{token}');" if self.path == "/write" else ""
                    body = f"""<!doctype html><title>Lurker profile test</title><script>
{initialize}
const request = new XMLHttpRequest();
request.open('GET', '/observed/' + localStorage.getItem('lurker-fixture'), false);
request.send();
</script>""".encode()
                else:
                    if self.path.startswith("/observed/"):
                        received.append(self.path.removeprefix("/observed/"))
                    body = b""
                self.send_response(200)
                self.send_header("Content-Type", "text/html; charset=utf-8")
                self.send_header("Cache-Control", "no-store")
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def exercise(self, phase):
        suite = self.suite
        profile = suite.mountpoint / "firefox-profile"
        if not os.path.ismount(suite.mountpoint):
            raise AssertionError("Firefox profile requires the owned encrypted mount")
        if phase == "write":
            profile.mkdir(mode=0o700)
        before = len(self.received)
        url = f"http://127.0.0.1:{self.server.server_port}/{phase}"
        log = suite.logs / f"firefox-{phase}.log"
        with log.open("wb") as stream:
            subprocess.run(
                [str(suite.options.firefox), "--headless", "--no-remote", "--profile", str(profile),
                 "--screenshot", str(suite.root / f"firefox-{phase}.png"), url],
                stdin=subprocess.DEVNULL, stdout=stream, stderr=subprocess.STDOUT,
                timeout=suite.options.timeout, check=True,
            )
        if self.received[before:] != [self.token]:
            raise AssertionError(f"Firefox did not return the expected profile data during {phase}; {log}")

    def close(self):
        self.server.shutdown()
        self.thread.join()
        self.server.server_close()


def snapshot(path):
    """Include inode identity as well as bytes; no traversal through symlinks."""
    entries = [path]
    if path.is_dir():
        entries.extend(sorted(path.rglob("*")))
    result = []
    for entry in entries:
        info = entry.lstat()
        if entry.is_symlink():
            data = ("symlink", os.readlink(entry))
        elif entry.is_file():
            data = ("file", info.st_size, file_hash(entry))
        else:
            data = ("directory",)
        result.append((str(entry.relative_to(path.parent)), info.st_dev, info.st_ino, data))
    # CGD parameters are essential separate recovery metadata.
    params = path.with_name(path.name + ".cgdparams")
    if params.exists():
        result.append((params.name, file_hash(params)))
    return result


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
        os.execv(args[0], args)
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
                    match = None if pending_prompt else PROMPT.search(transcript, consumed)
                    if match:
                        # This suite never supplies administrator credentials.
                        context = bytes(transcript[max(consumed, match.start() - 120):match.end()]).lower()
                        if b"[sudo]" in context or b"sudo password" in context:
                            raise RuntimeError("Unexpected authorization prompt; run with the explicit required privilege route")
                        if password is None:
                            raise RuntimeError("Unexpected credential prompt")
                        consumed = match.end()
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


class Suite:
    def __init__(self, options):
        self.options = options
        parent = Path(options.root).resolve()
        if not parent.is_dir():
            raise ValueError("--root must name an existing parent directory")
        self.root = Path(tempfile.mkdtemp(prefix="lurker-conformance-", dir=parent))
        self.logs = self.root / "logs"
        self.logs.mkdir(mode=0o700)
        self.source = self.root / ("vault 密碼 with spaces" if options.backend in DIRECTORIES else "vault 密碼 with spaces.img")
        self.mountpoint = self.root / "mount with spaces"
        self.mountpoint.mkdir(mode=0o700)
        self.other_mount = self.root / "duplicate mount"
        self.other_mount.mkdir(mode=0o700)
        self.password = "  Lurker 密碼 " + os.urandom(18).hex() + "  "
        self.wrong = "  Incorrect " + os.urandom(18).hex() + "  "
        self.payload = os.urandom(65536)
        self.mounted = False
        self.result = {"schema": SCHEMA, "suite": "native-cli-conformance", "status": "running",
                       "backend": options.backend, "cipher": options.cipher,
                       "detection_mode": "auto" if options.auto_detect else "explicit",
                       "readonly_expectation": "unsupported" if options.expect_readonly_unsupported else ("skipped" if options.skip_readonly else "supported"),
                       "host": platform.node(), "os": platform.system(), "release": platform.release(),
                       "architecture": platform.machine(), "binary": str(options.binary),
                       "binary_sha256": file_hash(options.binary), "source_commit": options.commit,
                       "root": str(self.root), "privilege_route": "unprivileged" if options.unprivileged else "native",
                       "started": time.time(), "cases": [],
                       "limitations": ["No VM shutdown/fault injection, dependency audit, keyfile, full-home or desktop qualification in this suite"]}
        atomic_json(self.root / "owner.json", {"schema": SCHEMA, "pid": os.getpid(), "uid": os.geteuid(), "created": time.time()})
        self.save()

    def save(self):
        atomic_json(self.root / "result.json", self.result)

    def args(self, action, *operands):
        kind = "auto" if self.options.auto_detect and action in {"mount", "unmount"} else self.options.backend
        args = [str(self.options.binary), "--color=never", action, "--type", kind]
        if self.options.unprivileged:
            args.append("--unprivileged")
        return args + list(map(str, operands))

    def cli(self, args, expected=True, password=None):
        name = self.result["cases"][-1]["name"]
        sequence = len(list(self.logs.glob(name + "-*.log")))
        log = self.logs / f"{name}-{sequence}.log"
        outcome = terminal(args, password, self.options.timeout, log)
        self.result["cases"][-1].setdefault("commands", []).append({"argv": args, "log": str(log), **outcome})
        if outcome["timed_out"]:
            raise AssertionError("CLI timed out; owned fixture retained")
        if (outcome["exit_code"] == 0) != expected:
            raise AssertionError(f"Expected {'success' if expected else 'failure'}, got exit {outcome['exit_code']}; {log}")
        return outcome

    def case(self, name, function=None, skip=None):
        started = time.monotonic()
        record = {"name": name, "status": "running", "started": time.time()}
        self.result["cases"].append(record)
        self.save()
        try:
            if skip:
                record.update(status="skipped", reason=skip)
            else:
                function()
                record["status"] = "passed"
        except Exception as error:
            record.update(status="failed", diagnostic=str(error))
            raise
        finally:
            record["duration_seconds"] = round(time.monotonic() - started, 3)
            self.save()

    def create_args(self):
        args = self.args("create", self.source)
        if self.options.backend not in DIRECTORIES:
            args.append(self.options.size_gb)
            if self.options.cipher:
                args.extend(["--cipher", self.options.cipher])
        return args

    def mount(self, password=None, readonly=False, target=None, expected=True):
        target = target or self.mountpoint
        args = self.args("mount", self.source, target)
        if readonly:
            args.append("--readonly")
        self.cli(args, expected=expected, password=password or self.password)
        if expected:
            if not os.path.ismount(target):
                raise AssertionError("CLI succeeded without a mount at the owned mountpoint")
            self.mounted = target == self.mountpoint

    def close(self):
        self.cli(self.args("unmount", self.mountpoint))
        if os.path.ismount(self.mountpoint):
            raise AssertionError("CLI reported success but owned mount remains attached")
        self.mounted = False

    def create(self):
        self.cli(self.create_args(), password=self.password)
        if not self.source.exists():
            raise AssertionError("Create succeeded without publishing the owned source")

    def no_overwrite(self):
        before = snapshot(self.source)
        self.cli(self.create_args(), expected=False, password=self.password)
        if snapshot(self.source) != before:
            raise AssertionError("Rejected create modified the existing source")

    def write_workload(self):
        payload = self.mountpoint / "profile 密碼.bin"
        with payload.open("xb") as stream:
            stream.write(self.payload)
            stream.flush()
            os.fsync(stream.fileno())
        self.atomic_workload()
        db = sqlite3.connect(self.mountpoint / "browser-profile.sqlite")
        try:
            mode = db.execute("PRAGMA journal_mode=WAL").fetchone()[0]
            if mode.lower() != "wal":
                raise AssertionError("SQLite did not enable WAL mode")
            db.execute("PRAGMA synchronous=FULL")
            db.execute("CREATE TABLE profile (key TEXT PRIMARY KEY, value BLOB)")
            db.execute("INSERT INTO profile VALUES (?, ?)", ("密碼", self.payload))
            db.commit()
            # Independent connection must see committed WAL data.
            reader = sqlite3.connect(self.mountpoint / "browser-profile.sqlite")
            try:
                assert reader.execute("SELECT value FROM profile WHERE key=?", ("密碼",)).fetchone()[0] == self.payload
            finally:
                reader.close()
            db.execute("PRAGMA wal_checkpoint(TRUNCATE)")
        finally:
            db.close()
        self.sync_dir(self.mountpoint)

    def atomic_workload(self):
        temporary = self.mountpoint / "atomic.tmp"
        published = self.mountpoint / "atomic.data"
        with temporary.open("xb") as stream:
            stream.write(b"initial-long-value")
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, published)
        with published.open("r+b") as stream:
            stream.truncate(7)
            stream.flush()
            os.fsync(stream.fileno())
        if published.read_bytes() != b"initial":
            raise AssertionError("Atomic rename/truncation workload did not persist bytes")
        self.sync_dir(self.mountpoint)

    @staticmethod
    def sync_dir(directory):
        descriptor = os.open(directory, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)

    def duplicate(self):
        self.mount(target=self.other_mount, expected=False)
        if os.path.ismount(self.other_mount):
            raise AssertionError("Rejected duplicate unexpectedly attached a second mount; retained")
        self.verify()

    def aliased_duplicate(self):
        alias = self.root / "source alias"
        if self.source.is_dir():
            alias.symlink_to(self.source, target_is_directory=True)
        else:
            os.link(self.source, alias)
            params = self.source.with_name(self.source.name + ".cgdparams")
            if params.exists():
                # Missing recovery params must not be the reason an alias fails.
                os.link(params, alias.with_name(alias.name + ".cgdparams"))
        self.cli(self.args("mount", alias, self.other_mount), expected=False, password=self.password)
        if os.path.ismount(self.other_mount):
            raise AssertionError("Aliased source bypassed duplicate attachment protection; retained")
        self.verify()

    def busy(self):
        # The cwd belongs to this child; terminate/wait before any retry.
        child = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(600)"], cwd=self.mountpoint,
                                 stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            self.cli(self.args("unmount", self.mountpoint), expected=False)
            if not os.path.ismount(self.mountpoint):
                raise AssertionError("Busy unmount detached live application storage")
            self.verify()
        finally:
            child.terminate()
            child.wait(timeout=10)

    def verify(self):
        if (self.mountpoint / "profile 密碼.bin").read_bytes() != self.payload:
            raise AssertionError("Durably written profile bytes did not survive")
        db = sqlite3.connect(self.mountpoint / "browser-profile.sqlite")
        try:
            if db.execute("PRAGMA integrity_check").fetchone()[0] != "ok":
                raise AssertionError("SQLite integrity check failed")
            if db.execute("SELECT value FROM profile WHERE key=?", ("密碼",)).fetchone()[0] != self.payload:
                raise AssertionError("SQLite committed profile record differs")
        finally:
            # Connection context managers commit/rollback; they do not close.
            # Close explicitly before testing filesystem drain/unmount.
            db.close()
        if (self.mountpoint / "atomic.data").read_bytes() != b"initial":
            raise AssertionError("Renamed/truncated file did not survive")

    def incorrect(self):
        original_mode = self.mountpoint.stat().st_mode & 0o7777
        self.mount(password=self.wrong, expected=False)
        if os.path.ismount(self.mountpoint):
            raise AssertionError("Incorrect credential left an attached mount")
        if platform.system() == "Darwin" and self.options.backend == "cryptomator":
            if self.mountpoint.stat().st_mode & 0o7777:
                raise AssertionError("Failed Cryptomator startup exposed a writable underlay")
            try:
                (self.mountpoint / "forbidden plaintext").write_bytes(b"owned test")
            except PermissionError:
                pass
            else:
                raise AssertionError("Failed Cryptomator startup allowed plaintext fallthrough")
            # Recovery restores the held inode, but still reports the original
            # failed backend exit. A following authenticated mount must work.
            self.cli(self.args("unmount", self.source), expected=False)
            if self.mountpoint.stat().st_mode & 0o7777 != original_mode:
                raise AssertionError("Explicit failed-startup recovery did not restore mountpoint mode")

    def readonly_unsupported(self):
        # A native limitation must fail closed, never silently attach writable
        # storage. Check the mount even when the CLI returns an unexpected code.
        try:
            self.mount(readonly=True, expected=False)
        finally:
            if os.path.ismount(self.mountpoint):
                raise AssertionError("Unsupported read-only request left storage attached; owned fixture retained")

    def readonly(self):
        self.mount(readonly=True)
        if (self.mountpoint / "profile 密碼.bin").read_bytes() != self.payload:
            raise AssertionError("Read-only mount changed profile contents")
        try:
            with (self.mountpoint / "forbidden-write").open("xb") as stream:
                stream.write(b"must not succeed")
                stream.flush()
                os.fsync(stream.fileno())
        except OSError as error:
            if error.errno != errno.EROFS:
                raise AssertionError("Read-only failure must be EROFS, not ordinary file permissions") from error
        else:
            raise AssertionError("Read-only storage accepted a write")
        self.close()

    def run(self):
        browser = None
        try:
            if getattr(self.options, "firefox", None):
                browser = BrowserProfile(self)
                self.result["firefox"] = {"path": str(self.options.firefox),
                                          "sha256": file_hash(self.options.firefox)}
            for name, function in [
                ("create", self.create), ("no-overwrite", self.no_overwrite),
                ("unlock", self.mount), ("application-io", self.write_workload),
                ("duplicate", self.duplicate), ("aliased-duplicate", self.aliased_duplicate), ("busy", self.busy),
                ("close", self.close), ("wrong-password", self.incorrect),
                ("reopen", self.mount), ("persistence", self.verify), ("reclose", self.close),
            ]:
                self.case(name, function)
                if browser and name in {"application-io", "persistence"}:
                    phase = "write" if name == "application-io" else "read"
                    self.case(f"firefox-profile-{phase}", lambda: browser.exercise(phase))
            self.case("readonly", self.readonly_unsupported if self.options.expect_readonly_unsupported else self.readonly,
                      skip=self.options.skip_readonly)
            self.result["status"] = "passed"
        except Exception as error:
            self.result.update(status="failed", diagnostic=str(error))
        finally:
            if browser:
                browser.close()
            self.result["finished"] = time.time()
            self.result["duration_seconds"] = round(self.result["finished"] - self.result["started"], 3)
            self.result["counts"] = {name: sum(c["status"] == name for c in self.result["cases"])
                                     for name in ("passed", "failed", "skipped")}
            self.result["retained"] = True
            self.result["mount_present"] = os.path.ismount(self.mountpoint)
            self.save()
        return self.result


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True, help="Exact CLI artifact; never search PATH")
    parser.add_argument("--backend", choices=sorted(BACKENDS), required=True)
    parser.add_argument("--cipher", choices=["aes", "serpent", "twofish"])
    parser.add_argument("--root", default="/tmp", help="Existing parent; a new private child is always allocated")
    parser.add_argument("--size-gb", default="0.25")
    parser.add_argument("--timeout", type=float, default=240)
    parser.add_argument("--commit", help="Source SHA supplied by the verified build coordinator")
    parser.add_argument("--firefox", type=Path, help="Run a fresh headless Firefox profile on the encrypted mount as the login user")
    parser.add_argument("--unprivileged", action="store_true", help="Use Linux's explicit VeraCrypt user route")
    parser.add_argument("--auto-detect", action="store_true",
                        help="Use --type auto for mount/unmount while creating the explicitly selected format")
    readonly = parser.add_mutually_exclusive_group()
    readonly.add_argument("--skip-readonly", metavar="REASON", help="Record an explicit unsupported-readonly exception")
    readonly.add_argument("--expect-readonly-unsupported", action="store_true",
                          help="Execute read-only mount and require rejection without any attached storage")
    options = parser.parse_args(argv)
    options.binary = options.binary.resolve()
    if not options.binary.is_file() or not os.access(options.binary, os.X_OK):
        parser.error("--binary must name an existing executable artifact")
    if options.firefox:
        options.firefox = options.firefox.resolve()
        if os.geteuid() == 0 or not options.firefox.is_file() or not os.access(options.firefox, os.X_OK):
            parser.error("--firefox requires an executable Firefox and a login-user run")
    if options.backend in DIRECTORIES and options.cipher:
        parser.error("Directory backend profiles do not accept --cipher")
    if options.timeout <= 0:
        parser.error("--timeout must be positive")
    if options.skip_readonly is not None and not options.skip_readonly.strip():
        parser.error("--skip-readonly requires a reason")
    if options.unprivileged and options.backend != "veracrypt":
        parser.error("--unprivileged is only for the Linux VeraCrypt user route")
    if platform.system() != "Darwin" and options.backend not in DIRECTORIES and not options.unprivileged and os.geteuid() != 0:
        parser.error("Native container tests require running this script as root; it never supplies sudo credentials")
    if options.backend in {"gocryptfs", "cryfs", "cryptomator"} and os.geteuid() == 0:
        parser.error("Userspace directory tests must run as the login user")
    return options


def main(argv=None):
    try:
        suite = Suite(parse_args(argv))
        result = suite.run()
        print(json.dumps({"schema": SCHEMA, "status": result["status"], "backend": result["backend"],
                          "counts": result["counts"], "duration_seconds": result["duration_seconds"],
                          "result": str(suite.root / "result.json"), "diagnostic": result.get("diagnostic")}))
        return 0 if result["status"] == "passed" else 1
    except (OSError, ValueError) as error:
        print(json.dumps({"schema": SCHEMA, "status": "blocked", "diagnostic": str(error)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
