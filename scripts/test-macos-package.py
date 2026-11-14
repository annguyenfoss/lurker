#!/usr/bin/env python3
"""Qualify an exact developer DMG on a Mac using private /tmp fixtures only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dmg", type=Path, required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--expected-uid", type=int, required=True)
    parser.add_argument("--resume", type=Path, help="Resume this exact package after preserving its failed attempt")
    args = parser.parse_args()
    assert platform.system() == "Darwin" and os.geteuid() == args.expected_uid != 0
    assert len(args.commit) == 40 and all(c in "0123456789abcdef" for c in args.commit)
    dmg = args.dmg.resolve(strict=True)
    assert hashlib.sha256(dmg.read_bytes()).hexdigest() == args.sha256
    previous = None
    if args.resume:
        assert not args.resume.is_symlink()
        root = args.resume.resolve(strict=True).parent
        assert root.parent == Path("/private/tmp") and root.name.startswith("lurker-macos-package-")
        assert root.stat().st_uid == os.geteuid() and root.stat().st_mode & 0o077 == 0
        previous = json.loads(args.resume.read_text())
        assert previous["source_commit"] == args.commit and previous["dmg_sha256"] == args.sha256
        assert previous["status"] == "failed"
    else:
        root = Path(tempfile.mkdtemp(prefix="lurker-macos-package-", dir="/private/tmp"))
    root.chmod(0o700)
    point = root / "dmg"
    point.mkdir(exist_ok=True)
    app = root / "Lurker.app"
    attached = False
    result = {"status": "failed", "source_commit": args.commit, "dmg_sha256": args.sha256,
              "os": platform.platform(), "architecture": platform.machine(), "uid": os.geteuid(),
              "harness_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "root": str(root), "app": str(app), "cases": []}
    environment = {k: v for k, v in os.environ.items()
                   if not k.startswith(("DYLD_", "LURKER_BACKEND_")) and k not in
                   {"JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "_JAVA_OPTIONS", "CLASSPATH", "LD_PRELOAD"}}
    environment["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    phase = "extract-and-verify-app"
    try:
        if previous:
            assert hashlib.sha256((app / "Contents/MacOS/lurker").read_bytes()).hexdigest() == previous["binary_sha256"]
            number = 1
            while (root / f"result-attempt-{number}.json").exists():
                number += 1
            saved = root / f"result-attempt-{number}.json"
            with saved.open("x") as stream:
                json.dump(previous, stream, indent=2)
            result["previous_attempt"] = str(saved)
        with (root / "extract.log").open("a" if previous else "w") as log:
            if not previous:
                subprocess.run(["/usr/bin/hdiutil", "attach", "-readonly", "-nobrowse", "-noautoopen",
                                "-mountpoint", str(point), str(dmg)], check=True, stdout=log, stderr=log, timeout=120)
                attached = True
                subprocess.run(["/usr/bin/ditto", str(point / "Lurker.app"), str(app)],
                               check=True, stdout=log, stderr=log, timeout=120)
                subprocess.run(["/usr/bin/hdiutil", "detach", str(point)], check=True,
                               stdout=log, stderr=log, timeout=60)
                attached = False
            subprocess.run(["/usr/bin/codesign", "--verify", "--deep", "--strict", str(app)],
                           check=True, stdout=log, stderr=log, timeout=60)
        binary = app / "Contents/MacOS/lurker"
        result["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
        scripts = Path(__file__).parent
        cases = [("apple", None, False, False), ("apple", None, False, True)]
        for cipher in ("aes", "serpent", "twofish"):
            cases += [("veracrypt", cipher, False, False), ("veracrypt", cipher, True, True)]
        for backend in ("gocryptfs", "cryfs", "cryptomator"):
            cases += [(backend, None, False, False), (backend, None, False, True)]
        for index, (backend, cipher, keyfile, auto) in enumerate(cases):
            phase = f"{backend}-{cipher or 'fixed'}-{'keyfile' if keyfile else 'password'}-{'auto' if auto else 'explicit'}"
            harness = scripts / ("test-veracrypt-keyfiles.py" if keyfile else "test-backends.py")
            if previous and index < len(previous["cases"]):
                prior = previous["cases"][index]
                if (prior["case"] == phase and prior["exit"] == 0 and prior["summary"]["status"] == "passed"
                        and prior["summary"]["counts"] == {"passed": 13, "failed": 0, "skipped": 0}
                        and prior["harness_sha256"] == hashlib.sha256(harness.read_bytes()).hexdigest()):
                    result["cases"].append(prior)
                    continue
            command = [sys.executable, str(harness), "--binary", str(binary), "--backend", backend,
                       "--commit", args.commit, "--root", str(root)]
            if cipher:
                command += ["--cipher", cipher]
            if auto:
                command += ["--auto-detect"]
            run = subprocess.run(command, env=environment, capture_output=True, text=True, timeout=900)
            log = root / f"suite-{index}{f'-retry-{number}' if previous else ''}.log"
            log.write_text(run.stdout + run.stderr)
            summary = json.loads(run.stdout.strip().splitlines()[-1])
            result["cases"].append({"case": phase, "exit": run.returncode, "summary": summary,
                                    "log": str(log), "harness_sha256": hashlib.sha256(harness.read_bytes()).hexdigest()})
            assert run.returncode == 0 and summary["status"] == "passed", str(log)
            assert summary["counts"] == {"passed": 13, "failed": 0, "skipped": 0}, str(summary)
        result["status"] = "passed"
    except Exception as error:
        result.update(failed_stage=phase, error=str(error)[:2048])
    finally:
        result["checks"] = sum(case["summary"]["counts"]["passed"] for case in result["cases"])
        if attached:
            closed = subprocess.run(["/usr/bin/hdiutil", "detach", str(point)], capture_output=True, timeout=60)
            result["dmg_cleanup_exit"] = closed.returncode
        (root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"status": result["status"], "checks": result.get("checks", 0),
                      "failed_stage": result.get("failed_stage"), "result": str(root / "result.json")}))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
