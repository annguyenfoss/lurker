#!/usr/bin/env python3
"""Native CLI lifecycle checks using private backends and fresh /tmp containers.

Run as root in a disposable Linux VM, never on a production workstation.
Only this job's images and recorded mountpoints are touched. Logs/results survive
failure; an unmount failure preserves its backing storage for manual recovery.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--types", nargs="+", choices=["luks", "luks1", "veracrypt"], default=["luks", "luks1", "veracrypt"])
    parser.add_argument("--ciphers", nargs="+", choices=["aes", "serpent", "twofish"], default=["aes", "serpent", "twofish"])
    parser.add_argument("--result", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=300, help="Per CLI operation deadline in seconds")
    args = parser.parse_args()
    if os.geteuid() != 0:
        parser.error("Run as root inside a disposable test VM.")
    cli = args.cli.resolve(strict=True)
    root = Path(tempfile.mkdtemp(prefix="lurker-bundled-test-", dir="/tmp"))
    root.chmod(0o700)
    key = root / "test.key"
    wrong = root / "incorrect.key"
    key.write_bytes(os.urandom(96))
    wrong.write_bytes(os.urandom(96))
    key.chmod(0o600)
    wrong.chmod(0o600)
    results = []
    opened = set()
    counter = 0
    uncertain = False

    def run(arguments, success=True):
        nonlocal counter, uncertain
        counter += 1
        log = root / f"command-{counter:03}.log"
        with log.open("wb") as output:
            try:
                process = subprocess.run([str(cli), *map(str, arguments)], stdin=subprocess.DEVNULL,
                                         stdout=output, stderr=subprocess.STDOUT, timeout=args.timeout)
            except subprocess.TimeoutExpired:
                uncertain = True  # A surviving backend may still own a staged file.
                raise
        if (process.returncode == 0) != success:
            raise RuntimeError(f"Unexpected CLI exit {process.returncode}; log={log}")

    try:
        for kind in args.types:
            for cipher in args.ciphers:
                started = time.monotonic()
                source = root / f"{kind}-{cipher} space.img"
                mountpoint = root / f"mount-{kind}-{cipher}"
                mountpoint.mkdir()
                case = {"backend": kind, "cipher": cipher, "status": "failed"}
                results.append(case)
                try:
                    run(["create", "--type", kind, "--cipher", cipher, "--key-file", key, source, "0.15"])
                    run(["create", "--type", kind, "--cipher", cipher, "--key-file", key, source, "0.15"], success=False)
                    run(["mount", "--type", kind, "--key-file", wrong, source, mountpoint], success=False)
                    run(["mount", "--type", kind, "--key-file", key, source, mountpoint])
                    opened.add(mountpoint)
                    # Actual application filesystem behavior, including WAL and fsync.
                    content = mountpoint / "unicode-你好.txt"
                    with content.open("wb") as output:
                        output.write(b"persisted through close and reopen\n")
                        output.flush()
                        os.fsync(output.fileno())
                    with sqlite3.connect(mountpoint / "state.sqlite") as database:
                        assert database.execute("PRAGMA journal_mode=WAL").fetchone()[0] == "wal"
                        database.execute("CREATE TABLE samples(value TEXT)")
                        database.execute("INSERT INTO samples VALUES ('persistent')")
                        database.commit()
                    database.close()
                    replacement = mountpoint / "replacement"
                    replacement.write_text("atomic")
                    replacement.rename(mountpoint / "renamed")
                    run(["mount", "--type", kind, "--key-file", key, source, mountpoint], success=False)
                    run(["unmount", "--type", kind, mountpoint])
                    opened.remove(mountpoint)
                    run(["mount", "--type", kind, "--readonly", "--key-file", key, source, mountpoint])
                    opened.add(mountpoint)
                    assert content.read_bytes() == b"persisted through close and reopen\n"
                    assert (mountpoint / "renamed").read_text() == "atomic"
                    with sqlite3.connect(f"file:{mountpoint}/state.sqlite?mode=ro&immutable=1", uri=True) as database:
                        assert database.execute("SELECT value FROM samples").fetchone()[0] == "persistent"
                    database.close()
                    try:
                        content.write_bytes(b"readonly must reject writes")
                    except OSError:
                        pass
                    else:
                        raise RuntimeError("Readonly filesystem permitted writes")
                    run(["unmount", "--type", kind, mountpoint])
                    opened.remove(mountpoint)
                    source.unlink()  # Reclaim tmpfs after verified close, before the next case.
                    case["status"] = "passed"
                except Exception as error:
                    case["error"] = str(error)
                case["duration_seconds"] = round(time.monotonic() - started, 3)
                if opened or uncertain:
                    break  # Preserve an unhealthy stack rather than piling on storage work.
            if opened or uncertain:
                break
    finally:
        for mountpoint in list(opened):
            try:
                run(["unmount", mountpoint])
                opened.remove(mountpoint)
            except Exception:
                pass
        # Check actual native resources as well as our last acknowledged CLI step.
        # A timeout can occur after mount/open but before its success was recorded.
        retained = []
        try:
            for line in Path("/proc/self/mountinfo").read_text().splitlines():
                mountpoint = line.split()[4]
                if mountpoint.startswith(str(root) + "/"):
                    retained.append(mountpoint)
            for backing in Path("/sys/class/block").glob("loop*/loop/backing_file"):
                if backing.read_text().strip().startswith(str(root) + "/"):
                    retained.append(str(backing.parent.parent))
        except OSError as error:
            uncertain = True
            retained.append("Resource inspection failed: " + str(error))
        all_passed = not uncertain and not retained and len(results) == len(args.types) * len(args.ciphers) and all(item["status"] == "passed" for item in results)
        result = {"schema_version": 1, "status": "passed" if all_passed else "failed",
                  "cases": results, "logs": str(root), "retained_mounts": list(map(str, opened)), "retained_resources": retained, "cleanup_uncertain": uncertain,
                  "separate_veracrypt_on_path": shutil.which("veracrypt")}
        args.result.parent.mkdir(parents=True, exist_ok=True)
        args.result.write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({"status": result["status"], "passed": sum(item["status"] == "passed" for item in results),
                          "total": len(args.types) * len(args.ciphers), "result": str(args.result)}))
        # Keep concise logs, but remove only this job's unmounted large containers/keys.
        if not opened and not retained and not uncertain:
            for path in [*root.glob("*.img"), key, wrong]:
                path.unlink(missing_ok=True)
    return 0 if all_passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
