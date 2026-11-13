#!/usr/bin/env python3
"""Deploy immutable source and supervise concise, reconnectable SSH test jobs."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tarfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = ROOT / "scripts/remote-test/hosts.json"
STATE = ROOT / "target/remote-test"
SSH = ["ssh", "-oBatchMode=yes", "-oConnectTimeout=10"]


def atomic_json(path, value):
    temp = path.with_suffix(".json.tmp")
    temp.write_text(json.dumps(value, indent=2) + "\n")
    os.replace(temp, path)


def ssh(host, code, args=(), data=None, timeout=60):
    command = shlex.join([host["python"], "-c", code, *map(str, args)])
    result = subprocess.run([*SSH, host["ssh"], command], input=data, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors="replace")[-2000:] or "remote command failed")
    return result.stdout


def resolve_python(host):
    command = ('for p in "$HOME/.local/bin/python3" /usr/local/bin/python3 '
               '/usr/pkg/bin/python3 /usr/bin/python3; do '
               '[ -x "$p" ] && "$p" -c \'import sys; print(sys.executable)\' && exit 0; done; exit 1')
    result = subprocess.run([*SSH, host["ssh"], command], capture_output=True, text=True, timeout=30)
    if result.returncode:
        raise RuntimeError("Python 3 unavailable or SSH failed: " + result.stderr[-600:])
    path = result.stdout.strip()
    if not path.startswith("/") or "\n" in path:
        raise RuntimeError("invalid remote Python path")
    return dict(host, python=path)


def source_payload(commit):
    data = subprocess.check_output(["git", "archive", "--format=tar", commit], cwd=ROOT)
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        for entry in archive.getmembers():
            if entry.isfile():
                files[entry.name] = {"sha256": hashlib.sha256(archive.extractfile(entry).read()).hexdigest()}
            elif entry.issym():
                files[entry.name] = {"symlink": entry.linkname}
            elif not entry.isdir():
                raise ValueError("unsupported archive member " + entry.name)
    return data, {"commit": commit, "archive_sha256": hashlib.sha256(data).hexdigest(), "files": files}


def stages_for(suite):
    stages = [
        {"name": "backend-build", "skip": "not requested by this suite"},
        {"name": "rust-check", "commands": [["cargo", "check", "-p", "lurker-core", "-p", "lurker-cli", "-p", "lurker-helper"]], "timeout": 1800},
        {"name": "rust-tests", "commands": [["cargo", "test", "-p", "lurker-core", "-p", "lurker-cli", "-p", "lurker-helper", "--no-fail-fast"]], "timeout": 1800},
    ]
    if suite == "workspace":
        stages[1]["commands"] = [["cargo", "check", "--workspace"]]
        stages[2]["commands"] = [["cargo", "test", "--workspace", "--no-fail-fast"]]
    stages += [{"name": "packaging", "skip": "not requested by build-only suite"},
               {"name": "installed-artifact-tests", "skip": "build-only suite does not qualify runtime encryption support"}]
    return stages


def validate_spec(spec):
    for artifact in spec.get("artifacts", []):
        path = Path(artifact)
        if path.is_absolute() or ".." in path.parts or not path.parts:
            raise ValueError("artifact must be relative to CARGO_TARGET_DIR")
    names = set()
    for stage in spec["stages"]:
        if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", stage["name"]) or stage["name"] in names:
            raise ValueError("invalid or duplicate stage name")
        names.add(stage["name"])
        variants = [bool(stage.get(key)) for key in ("commands", "skip", "blocked")]
        if sum(variants) != 1:
            raise ValueError("stage needs exactly one of commands/skip/blocked")
        if stage.get("commands"):
            for command in stage["commands"]:
                if not isinstance(command, list) or not command or not all(isinstance(part, str) for part in command):
                    raise ValueError("commands must be argv arrays")
        if stage.get("timeout", 1800) <= 0:
            raise ValueError("stage timeout must be positive")


DEPLOY = r'''
import hashlib,io,json,os,pathlib,stat,subprocess,sys,tarfile,tempfile
meta=json.loads(sys.argv[1]); root=pathlib.Path(tempfile.gettempdir())/('lurker-matrix-'+str(os.getuid()))
root.mkdir(mode=0o700,exist_ok=True)
info=root.lstat()
if not stat.S_ISDIR(info.st_mode) or info.st_uid!=os.getuid() or stat.S_IMODE(info.st_mode)!=0o700: raise ValueError('unsafe run root')
job=root/'runs'/meta['job_id']
job.mkdir(parents=True, exist_ok=False)
os.chmod(job,0o700)
data=sys.stdin.buffer.read()
if hashlib.sha256(data).hexdigest()!=meta['source_manifest']['archive_sha256']: raise ValueError('archive hash mismatch')
source=job/'source';source.mkdir()
with tarfile.open(fileobj=io.BytesIO(data)) as archive:
 for item in archive.getmembers():
  path=pathlib.PurePosixPath(item.name)
  if path.is_absolute() or '..' in path.parts or not (item.isfile() or item.isdir() or item.issym()): raise ValueError('unsafe archive entry')
  if item.issym():
   target=pathlib.PurePosixPath(item.linkname)
   if target.is_absolute() or '..' in target.parts: raise ValueError('unsafe source symlink')
 archive.extractall(source,filter='data') if sys.version_info>=(3,12) else archive.extractall(source)
(job/'spec.json').write_text(json.dumps(meta['spec']))
(job/'source-manifest.json').write_text(json.dumps(meta['source_manifest']))
runner=source/'scripts/remote-test/runner.py'
if not runner.is_file(): raise ValueError('runner absent from requested commit')
with (job/'supervisor.log').open('wb') as log:
 process=subprocess.Popen([sys.executable,str(runner),str(job)],stdin=subprocess.DEVNULL,stdout=log,stderr=log,start_new_session=True,close_fds=True)
signature=subprocess.run(['ps','-p',str(process.pid),'-o','lstart='],capture_output=True,text=True).stdout.strip()
(job/'launcher.json').write_text(json.dumps({'pid':process.pid,'signature':signature}))
print(json.dumps({'job':str(job),'pid':process.pid}))
'''

INSPECT = r'''
import json,pathlib,subprocess,sys
job=pathlib.Path(sys.argv[1]);result=job/'result.json'
state=json.loads(result.read_text()) if result.exists() else {'status':'incomplete','diagnostic':'runner has not published a result','job':str(job)}
if state['status'] in ('running','incomplete'):
 launcher=job/'launcher.json'
 if launcher.exists():
  identity=json.loads(launcher.read_text())
  observed=subprocess.run(['ps','-p',str(identity['pid']),'-o','lstart='],capture_output=True,text=True).stdout.strip()
  if not observed or (identity.get('signature') and observed!=identity['signature']):
   state.update(status='incomplete',runner_terminal=True,diagnostic='runner process missing or replaced; inspect retained logs/resources before rerunning')
print(json.dumps(state))
'''



def deploy(host, job_id, run_id, commit, args, data, manifest, override):
    record = {"job_id": job_id, "host": host, "status": "deploying"}
    try:
        resolved = resolve_python(host)
        record["host"] = resolved
        spec = {"schema": 1, "job_id": job_id, "run_id": run_id, "commit": commit,
                "host": host, "suite": args.suite, "minimum_free_bytes": args.minimum_free_mib * 1024 * 1024,
                "build_jobs": args.build_jobs, "stages": stages_for(args.suite)}
        if override:
            spec.update(override)
            # Run/source identity cannot be overridden by a stage recipe.
            spec.update(job_id=job_id, run_id=run_id, commit=commit, host=host)
        validate_spec(spec)
        metadata = {"job_id": job_id, "spec": spec, "source_manifest": manifest}
        record.update(json.loads(ssh(resolved, DEPLOY, [json.dumps(metadata)], data=data, timeout=120)))
        record["status"] = "launched"
    except Exception as error:
        record.update(status="blocked", diagnostic=str(error))
    return record


def start(args):
    inventory = json.loads(args.inventory.read_text())
    targets = args.hosts.split(",") if args.hosts else [key for key, host in inventory.items() if host.get("primary")]
    unknown = set(targets) - set(inventory)
    if unknown:
        raise ValueError("unknown hosts: " + ", ".join(sorted(unknown)))
    if len(set(targets)) != len(targets):
        raise ValueError("duplicate host selection")
    commit = subprocess.check_output(["git", "rev-parse", args.commit + "^{commit}"], cwd=ROOT, text=True).strip()
    data, source_manifest = source_payload(commit)
    if "scripts/remote-test/runner.py" not in source_manifest["files"]:
        raise ValueError("requested commit does not contain the remote runner")
    run_id = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime()) + "-" + uuid.uuid4().hex[:8]
    directory = STATE / run_id
    directory.mkdir(parents=True)
    override = json.loads(args.spec.read_text()) if args.spec else None
    manifest = {"schema": 1, "run_id": run_id, "commit": commit, "suite": override.get("suite", args.suite) if override else args.suite,
                "created": time.time(), "jobs": [], "concurrency": args.concurrency}
    atomic_json(directory / "manifest.json", manifest)
    (directory / "source.tar").write_bytes(data)
    atomic_json(directory / "source-manifest.json", source_manifest)
    manifest["jobs"] = [{"job_id": run_id + "-" + name, "host": inventory[name], "status": "queued"} for name in targets]
    manifest["options"] = {"suite": args.suite, "minimum_free_mib": args.minimum_free_mib,
                           "build_jobs": args.build_jobs, "override": override}
    atomic_json(directory / "manifest.json", manifest)
    with (directory / "dispatcher.log").open("wb") as log:
        process = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "dispatch", run_id],
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                   start_new_session=True, close_fds=True)
    print("run_id=" + run_id)
    print("dispatcher_pid=" + str(process.pid))
    print("collect: python scripts/remote-test.py collect " + run_id)
    return 0


def dispatch(args):
    directory, manifest = read_run(args.run_id)
    # Prevent two local supervisors from launching queued jobs concurrently.
    (directory / "dispatch-claim").mkdir()
    data = (directory / "source.tar").read_bytes()
    source_manifest = json.loads((directory / "source-manifest.json").read_text())
    options = argparse.Namespace(**manifest["options"])
    active = []
    pending = list(manifest["jobs"])
    while pending or active:
        while pending and len(active) < manifest["concurrency"]:
            record = pending.pop(0)
            if (directory / (record["job_id"] + ".cancel")).exists():
                record["status"] = "cancelled"
                atomic_json(directory / "manifest.json", manifest)
                continue
            deployed = deploy(record["host"], record["job_id"], manifest["run_id"], manifest["commit"],
                              options, data, source_manifest, options.override)
            record.update(deployed)
            atomic_json(directory / "manifest.json", manifest)
            if record["status"] == "launched":
                if (directory / (record["job_id"] + ".cancel")).exists():
                    ssh(record["host"], "import pathlib,sys;(pathlib.Path(sys.argv[1])/'cancel').touch()", [record["job"]])
                active.append(record)
        retained = []
        for record in active:
            try:
                state = json.loads(ssh(record["host"], INSPECT, [record["job"]]))
                if state["status"] == "running" or (state["status"] == "incomplete" and not state.get("runner_terminal")):
                    retained.append(record)
            except Exception:
                retained.append(record)  # timeouts never launch a duplicate
        active = retained
        if pending or active:
            time.sleep(5)
    return 0


def read_run(run_id):
    if not re.fullmatch(r"[a-zA-Z0-9-]+", run_id):
        raise ValueError("invalid run ID")
    directory = STATE / run_id
    return directory, json.loads((directory / "manifest.json").read_text())


def collect(args):
    directory, manifest = read_run(args.run_id)
    states = []
    for job in manifest["jobs"]:
        if job["status"] != "launched":
            result = job
        else:
            try:
                result = json.loads(ssh(job["host"], INSPECT, [job["job"]]))
                if result.get("commit") not in (None, manifest["commit"]):
                    raise ValueError("result source commit differs from manifest")
            except Exception as error:
                result = {"status": "incomplete", "diagnostic": str(error)}
        result["job_id"] = job["job_id"]
        states.append(result)
        counts = {key: sum(stage.get("test_counts", {}).get(key, 0) for stage in result.get("stages", []))
                  for key in ("passed", "failed", "skipped")}
        print("{} {} {} tests={}/{}/{} {}".format(job["host"]["ssh"], manifest["commit"][:10], result["status"],
              *counts.values(), result.get("failing_stage", "")))
        if result.get("diagnostic") and result["status"] != "passed":
            print(str(result["diagnostic"])[-8192:])
    atomic_json(directory / "result.json", {"schema": 1, "run_id": manifest["run_id"], "commit": manifest["commit"], "jobs": states})
    return 0 if states and all(item["status"] == "passed" for item in states) else 1


def control(args):
    directory, manifest = read_run(args.run_id)
    jobs = [job for job in manifest["jobs"] if args.host is None or job["job_id"].endswith("-" + args.host)]
    if not jobs:
        raise ValueError("no matching job")
    for job in jobs:
        if args.command == "cancel":
            (directory / (job["job_id"] + ".cancel")).touch()
        if job["status"] != "launched":
            continue
        if args.command == "cancel":
            code = "import pathlib,sys; (pathlib.Path(sys.argv[1])/'cancel').touch(); print('cancellation requested')"
            print(ssh(job["host"], code, [job["job"]]).decode().strip())
        else:
            if not re.fullmatch(r"[a-z][a-z0-9-]{0,63}", args.stage):
                raise ValueError("invalid stage name")
            code = ("import pathlib,sys; p=pathlib.Path(sys.argv[1])/(sys.argv[2]+'.log'); "
                    "f=p.open('rb'); f.seek(max(0,p.stat().st_size-8192)); "
                    "print('\\n'.join(f.read().decode(errors='replace').splitlines()[-40:]))")
            print(ssh(job["host"], code, [job["job"], args.stage]).decode())
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    launch = sub.add_parser("start")
    launch.add_argument("--hosts", help="inventory names, comma-separated; default primary targets")
    launch.add_argument("--inventory", type=Path, default=INVENTORY)
    launch.add_argument("--commit", default="HEAD")
    launch.add_argument("--suite", choices=("core", "workspace"), default="core")
    launch.add_argument("--spec", type=Path, help="JSON stage recipe; no secrets")
    launch.add_argument("--concurrency", type=int, default=2)
    launch.add_argument("--build-jobs", type=int, default=2)
    launch.add_argument("--minimum-free-mib", type=int, default=1024)
    for name in ("collect", "inspect", "resume", "dispatch"):
        command = sub.add_parser(name)
        command.add_argument("run_id")
    for name in ("cancel", "log"):
        command = sub.add_parser(name)
        command.add_argument("run_id")
        command.add_argument("--host")
        if name == "log":
            command.add_argument("stage")
    args = parser.parse_args()
    try:
        if args.command == "start":
            if args.concurrency < 1 or args.build_jobs < 1 or args.minimum_free_mib < 1:
                raise ValueError("concurrency, build jobs and free space must be positive")
            return start(args)
        if args.command == "dispatch":
            return dispatch(args)
        if args.command in ("collect", "inspect", "resume"):
            return collect(args)
        return control(args)
    except (ValueError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
