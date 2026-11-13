#!/usr/bin/env python3
"""Cross-UID kernel/userspace exclusion on one fresh disposable VeraCrypt file.

Run as root on a disposable Linux VM. The login account owns /tmp fixtures.
No existing volume, forced unmount or filesystem repair is used.
"""
import argparse
import json
import os
from pathlib import Path
import pwd
import re
import subprocess
import tempfile
import time


def is_mounted(path):
    # Root cannot stat another user's FUSE mount without allow_root. The native
    # mount table identifies it without reading the mounted plaintext.
    expected = os.fsencode(str(path))
    for line in Path("/proc/self/mountinfo").read_bytes().splitlines():
        field = line.split()[4]
        decoded = re.sub(rb"\\([0-7]{3})", lambda match: bytes([int(match.group(1), 8)]), field)
        if decoded == expected:
            return True
    return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, required=True)
    parser.add_argument("--user", default="adjutant")
    parser.add_argument("--result", type=Path, required=True)
    parser.add_argument("--race-rounds", type=int, default=3)
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error("Run as root inside a disposable VM")
    if args.race_rounds < 1:
        parser.error("--race-rounds must be positive")
    prefix = args.prefix.resolve(strict=True)
    cli = prefix / "bin/lurker"
    crypto = prefix / "libexec/lurker/backends/lurker-veracrypt"
    for path in [cli, *cli.parents]:
        metadata = path.stat()
        if metadata.st_uid != 0 or metadata.st_mode & 0o022:
            parser.error("Root tests require an administrator-controlled executable and ancestors")
    user = pwd.getpwnam(args.user)
    root = Path(tempfile.mkdtemp(prefix="lurker-cross-mode-", dir="/tmp"))
    os.chown(root, user.pw_uid, user.pw_gid)
    key = root / "binary key.bin"
    key.write_bytes(os.urandom(128)); key.chmod(0o600)
    os.chown(key, user.pw_uid, user.pw_gid)
    source = root / "source 密碼.hc"
    alias = root / "hard-link alias.hc"
    user_mount, kernel_mount = root / "user mount", root / "kernel mount"
    auxiliary = root / "direct-native"
    for path in [user_mount, kernel_mount, auxiliary]:
        path.mkdir(mode=0o700); os.chown(path, user.pw_uid, user.pw_gid)
    env = os.environ.copy(); env["LC_ALL"] = "C.UTF-8"
    for name in ["LURKER_BACKEND_DIR", "LURKER_USERSPACE_DIR", "LD_LIBRARY_PATH", "XDG_RUNTIME_DIR"]:
        env.pop(name, None)
    report = {"schema": 1, "suite": "linux-cross-mode-source-exclusion", "status": "running",
              "source_commit": json.loads((prefix / "share/lurker/manifest.json").read_text())["source_commit"],
              "root": str(root), "cases": [], "started": time.time()}
    opened = []
    sequence = 0

    def save():
        args.result.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.result.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(report, indent=2) + "\n")
        temporary.chmod(0o600); temporary.replace(args.result)

    def command(route, action, path=source, readonly=False):
        result = [str(cli), action, "--type", "veracrypt"]
        if route == "user": result.append("--unprivileged")
        if action in ["create", "mount"]: result.extend(["--key-file", str(key)])
        if readonly: result.append("--readonly")
        result.append(str(path))
        if action == "create": result.append("0.15")
        if action == "mount": result.append(str(user_mount if route == "user" else kernel_mount))
        return ["runuser", "-u", args.user, "--", *result] if route == "user" else result

    def execute(arguments, expected=True):
        nonlocal sequence
        sequence += 1
        with (root / f"command-{sequence:03}.log").open("wb") as log:
            result = subprocess.run(arguments, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, env=env, timeout=120)
        if (result.returncode == 0) != expected:
            raise RuntimeError(f"Unexpected exit {result.returncode}; command-{sequence:03}.log")

    def mount(route, path=source, readonly=False):
        execute(command(route, "mount", path, readonly))
        point = user_mount if route == "user" else kernel_mount
        if not is_mounted(point): raise RuntimeError("CLI succeeded without attaching its fixture")
        opened.append((route, point))

    def close():
        while opened:
            route, point = opened[-1]
            execute(command(route, "unmount", point))
            if is_mounted(point): raise RuntimeError("Fixture remained mounted after close")
            opened.pop()

    def rejected(route, path=source, readonly=False):
        execute(command(route, "mount", path, readonly), expected=False)
        point = user_mount if route == "user" else kernel_mount
        if is_mounted(point): raise RuntimeError("Rejected duplicate attached storage")

    def case(name, function):
        started = time.monotonic()
        entry = {"name": name, "status": "running"}; report["cases"].append(entry); save()
        try:
            function(); entry["status"] = "passed"
        except Exception as error:
            entry.update(status="failed", diagnostic=str(error)); raise
        finally:
            entry["duration_seconds"] = round(time.monotonic() - started, 3); save()

    def active_user():
        mount("user")
        rejected("kernel"); rejected("kernel", alias)
        moved = root / "renamed source.hc"
        source.rename(moved)
        try: rejected("kernel", moved)
        finally: moved.rename(source)
        close()

    def active_kernel():
        mount("kernel")
        rejected("user"); rejected("user", alias)
        source.unlink()
        try:
            rejected("user", alias)
            direct_env = dict(env, TMPDIR=str(auxiliary))
            arguments = ["runuser", "-u", args.user, "--", "env", "TMPDIR=" + str(auxiliary), str(crypto), "-t", "--mount", "--filesystem=none",
                         "--mount-options=nokernelcrypto", "--stdin", "--non-interactive", "--pim=0", "--protect-hidden=no", "-k", str(key), str(alias)]
            # Bypass the Rust preflight to exercise the post-flock native check.
            with (root / "direct-native.log").open("wb") as log:
                process = subprocess.run(arguments, input=b"\n", stdout=log, stderr=subprocess.STDOUT, env=direct_env, timeout=60)
            if process.returncode == 0: raise RuntimeError("Native driver bypassed a deleted kernel backing inode")
            if any(is_mounted(path) for path in auxiliary.iterdir()):
                raise RuntimeError("Rejected direct native request left a decrypted mount")
        finally: os.link(alias, source)
        close()

    def readonly_order(first, second):
        mount(first, readonly=True); rejected(second, readonly=True); close()

    def race():
        logs = [(root / "race-kernel.log").open("wb"), (root / "race-user.log").open("wb")]
        processes = [subprocess.Popen(command("kernel", "mount", alias), stdin=subprocess.DEVNULL, stdout=logs[0], stderr=subprocess.STDOUT, env=env),
                     subprocess.Popen(command("user", "mount"), stdin=subprocess.DEVNULL, stdout=logs[1], stderr=subprocess.STDOUT, env=env)]
        try:
            codes = [process.wait(timeout=120) for process in processes]
        finally:
            for process in processes:
                if process.poll() is None:
                    process.terminate()
                    try: process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill(); process.wait()
            for log in logs: log.close()
        mounted = [(route, point) for route, point in [("kernel", kernel_mount), ("user", user_mount)] if is_mounted(point)]
        opened.extend(mounted)
        if sum(code == 0 for code in codes) != 1 or len(mounted) != 1:
            raise RuntimeError(f"Concurrent startup did not have exactly one winner: exits={codes}, mounts={len(mounted)}")
        close()

    try:
        case("create-disposable-user-container", lambda: execute(command("user", "create")))
        os.link(source, alias)
        case("user-first-root-alias-and-rename-rejected", active_user)
        case("kernel-first-user-alias-and-deleted-backing-rejected", active_kernel)
        case("user-readonly-first", lambda: readonly_order("user", "kernel"))
        case("kernel-readonly-first", lambda: readonly_order("kernel", "user"))
        for index in range(args.race_rounds): case(f"concurrent-root-user-startup-{index + 1}", race)
        report["status"] = "passed"
    except Exception as error:
        report.update(status="failed", diagnostic=str(error))
    finally:
        for route, point in [("kernel", kernel_mount), ("user", user_mount)]:
            if is_mounted(point) and (route, point) not in opened: opened.append((route, point))
        try: close()
        except Exception as error: report["cleanup_error"] = str(error)
        report["retained_mounts"] = [str(point) for _, point in opened]
        report["finished"] = time.time(); report["duration_seconds"] = round(report["finished"] - report["started"], 3); save()
    print(json.dumps({"status": report["status"], "passed": sum(item["status"] == "passed" for item in report["cases"]), "result": str(args.result)}))
    return report["status"] != "passed"


if __name__ == "__main__":
    raise SystemExit(main())
