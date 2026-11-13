#!/usr/bin/env python3
"""Detached job runner. Source tree and spec are immutable job inputs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import time

SCHEMA = 1
TERMINAL = {"passed", "failed", "blocked", "cancelled", "incomplete"}


def atomic_json(path, value):
    path = Path(path)
    temp = path.with_suffix(".json.tmp")
    temp.write_text(json.dumps(value, indent=2) + "\n")
    os.replace(temp, path)


def digest(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def excerpt(path, lines=40, byte_limit=8192):
    # Tail retains actionable compiler/test failures; bounded reads retain memory.
    with open(path, "rb") as stream:
        stream.seek(max(0, os.fstat(stream.fileno()).st_size - byte_limit))
        data = stream.read(byte_limit).decode("utf-8", "replace")
    return "\n".join(data.splitlines()[-lines:])


def verify_source(source, manifest):
    expected = manifest["files"]
    for name, item in expected.items():
        path = source / name
        if item.get("symlink") is not None:
            if not path.is_symlink() or os.readlink(path) != item["symlink"]:
                raise ValueError("source symlink differs: " + name)
        elif not path.is_file() or path.is_symlink() or digest(path) != item["sha256"]:
            raise ValueError("source file differs: " + name)
    # Cargo build products and caches live outside the source tree.
    actual = {str(p.relative_to(source)) for p in source.rglob("*") if p.is_file() or p.is_symlink()}
    if actual != set(expected):
        raise ValueError("source tree has unexpected/missing entries")


def selected_environment(spec, job):
    env = os.environ.copy()
    home = Path.home()
    mode = spec["host"].get("rust", "rustup")
    if mode == "rustup":
        cargo = home / ".cargo/bin/cargo"
        rustc = home / ".cargo/bin/rustc"
    else:
        cargo = Path(shutil.which("cargo") or "/missing/cargo")
        rustc = Path(shutil.which("rustc") or "/missing/rustc")
    env["PATH"] = str(cargo.parent) + os.pathsep + env.get("PATH", "")
    # Keep per-host cached outputs separate from deployed source. Keyed by actual
    # compiler identity, target, and complete suite/build flags specification.
    versions = {}
    for name, executable in (("cargo", cargo), ("rustc", rustc)):
        result = subprocess.run([str(executable), "--version"], capture_output=True, text=True, timeout=30)
        if result.returncode:
            raise ValueError(name + " unavailable: " + result.stderr.strip()[:300])
        versions[name] = {"path": str(executable), "version": result.stdout.strip()}
    key = hashlib.sha256(json.dumps({"versions": versions, "os": platform.platform(),
                                    "stages": spec["stages"]}, sort_keys=True).encode()).hexdigest()[:20]
    cache = job.parents[1] / "cache" / key
    cache.mkdir(parents=True, exist_ok=True)
    env["CARGO_TARGET_DIR"] = str(cache / "target")
    env["CARGO_BUILD_JOBS"] = str(spec.get("build_jobs", 2))
    env["LURKER_TEST_JOB_DIR"] = str(job)
    env["LURKER_TEST_SOURCE_COMMIT"] = spec["commit"]
    return env, versions, cache


def terminate_group(process):
    # Only signal a child launched in its own group and not yet reaped. No stored
    # PID is used after reconnecting, so PID reuse cannot target unrelated work.
    if process.poll() is not None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()


def execute(stage, source, job, env):
    record = {"name": stage["name"], "status": "running", "started": time.time(),
              "test_counts": {"passed": 0, "failed": 0, "skipped": 0}}
    log = job / (stage["name"] + ".log")
    record["log"] = str(log)
    deadline = time.monotonic() + stage.get("timeout", 1800)
    with log.open("wb") as output:
        for command in stage["commands"]:
            command = [part.replace("{python}", sys.executable) for part in command]
            process = subprocess.Popen(command, cwd=source, env=env, stdin=subprocess.DEVNULL,
                                       stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
            while process.poll() is None:
                if (job / "cancel").exists() or time.monotonic() >= deadline:
                    record["status"] = "cancelled" if (job / "cancel").exists() else "timeout"
                    terminate_group(process)
                    break
                time.sleep(0.2)
            record["exit_code"] = process.wait()
            if record.get("status") in ("cancelled", "timeout") or record["exit_code"] != 0:
                if record["status"] == "running":
                    record["status"] = "failed"
                break
        else:
            record["status"] = "passed"
            record["exit_code"] = 0
    record["duration_seconds"] = round(time.time() - record.pop("started"), 3)
    record["log_sha256"] = digest(log)
    # Rust reports counts itself. Never invent test counts from command success.
    counts = {key: 0 for key in ("passed", "failed", "skipped")}
    with log.open(errors="replace") as stream:
        for line in stream:
            match = re.search(r"test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;", line)
            if match:
                for key, amount in zip(counts, match.groups()):
                    counts[key] += int(amount)
    record["test_counts"] = counts
    if record["status"] != "passed":
        record["diagnostic"] = excerpt(log)
    return record


def run_job(job):
    job = Path(job).resolve()
    spec = json.loads((job / "spec.json").read_text())
    # mkdir is an atomic claim that survives disconnects. A crashed job is never
    # restarted in place; an operator starts a new job after inspecting evidence.
    try:
        (job / "claimed").mkdir()
    except FileExistsError:
        return 2
    source = job / "source"
    result = {"schema": SCHEMA, "run_id": spec["run_id"], "job_id": spec["job_id"],
              "commit": spec["commit"], "host": spec["host"], "suite": spec["suite"],
              "platform": {"os": platform.system(), "release": platform.release(),
                           "kernel": platform.version(), "arch": platform.machine()},
              "status": "running", "started": time.time(), "stages": [],
              "privilege_route": spec.get("privilege_route", "none"),
              "inner_filesystem": spec.get("inner_filesystem"), "artifacts": []}
    atomic_json(job / "result.json", result)
    active_lock = None
    preflight_stage = "prerequisites"
    try:
        if platform.system() != spec["host"]["os"]:
            raise ValueError("host OS does not match inventory")
        if platform.machine() != spec["host"]["arch"]:
            raise ValueError("host architecture does not match inventory")
        free = shutil.disk_usage(job).free
        if free < spec.get("minimum_free_bytes", 1024 * 1024 * 1024):
            raise ValueError("insufficient free disk: " + str(free))
        env, versions, cache = selected_environment(spec, job)
        result["stages"].append({"name": "prerequisites", "status": "passed", "free_bytes": free})
        preflight_stage = "source-verification"
        verify_source(source, json.loads((job / "source-manifest.json").read_text()))
        result["stages"].append({"name": "source-verification", "status": "passed"})
        preflight_stage = None
        result["toolchains"] = versions
        result["cache"] = str(cache)
        # Serialize same-host jobs: cached compiler outputs and storage tests must
        # not conflict. Keep this lock on abnormal exit for explicit reconciliation.
        active_lock = job.parents[1] / "active-job"
        try:
            active_lock.mkdir()
        except FileExistsError:
            active_lock = None
            raise ValueError("host has another active/stale job; inspect active-job owner")
        (active_lock / "owner.json").write_text(json.dumps({"job": str(job), "pid": os.getpid()}))
        for stage in spec["stages"]:
            if (job / "cancel").exists():
                result["status"] = "cancelled"
                break
            if stage.get("skip"):
                record = {"name": stage["name"], "status": "skipped", "reason": stage["skip"]}
            elif stage.get("blocked"):
                record = {"name": stage["name"], "status": "blocked", "reason": stage["blocked"]}
            else:
                record = execute(stage, source, job, env)
            result["stages"].append(record)
            atomic_json(job / "result.json", result)
            if record["status"] not in ("passed", "skipped"):
                result["status"] = "blocked" if record["status"] == "blocked" else "failed"
                result["failing_stage"] = record["name"]
                result["diagnostic"] = record.get("diagnostic", record.get("reason"))
                break
        else:
            result["status"] = "passed"
        for relative in spec.get("artifacts", []):
            path = Path(env["CARGO_TARGET_DIR"]) / relative
            if path.is_file():
                stored = job / "artifacts" / relative
                stored.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, stored)
                result["artifacts"].append({"path": str(stored), "sha256": digest(stored), "bytes": stored.stat().st_size})
    except Exception as error:
        result["status"] = "blocked" if preflight_stage or not result["stages"] else "failed"
        result["diagnostic"] = str(error)[:8192]
        if preflight_stage:
            result["stages"].append({"name": preflight_stage, "status": "blocked", "reason": result["diagnostic"]})
            result["failing_stage"] = preflight_stage
    finally:
        if active_lock is not None:
            (active_lock / "owner.json").unlink(missing_ok=True)
            active_lock.rmdir()
        result["duration_seconds"] = round(time.time() - result["started"], 3)
        result["finished"] = time.time()
        atomic_json(job / "result.json", result)
    print(json.dumps({"status": result["status"], "result": str(job / "result.json")}))
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("job", type=Path)
    sys.exit(run_job(parser.parse_args().job))
