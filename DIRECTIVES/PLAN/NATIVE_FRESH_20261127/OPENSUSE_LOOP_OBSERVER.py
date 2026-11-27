#!/usr/bin/env python3
"""Sample only owned login-controller descendants; never record credential argv."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import time


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--controller', type=int, required=True)
    p.add_argument('--prefix', type=Path, required=True)
    p.add_argument('--root', type=Path, required=True)
    p.add_argument('--timeout', type=int, default=300)
    a = p.parse_args()
    if os.geteuid() != 0 or os.uname().nodename != 'opensuse-core-01':
        p.error('Owned openSUSE administrator required')
    if not 1 <= a.timeout <= 600:
        p.error('Bound observer to at most ten minutes')
    controller = Path('/proc') / str(a.controller)
    if controller.stat().st_uid != 1300:
        p.error('Controller must be the owned login user')
    helper = (a.prefix / 'bin/lurker-helper').resolve(strict=True)
    for path in [helper, *helper.parents]:
        st = path.stat()
        if st.st_uid or st.st_mode & 0o022:
            p.error('Helper prefix must be administrator-controlled')
    a.root.mkdir(mode=0o700, parents=True, exist_ok=False)
    helper_token = os.fsencode(helper)
    samples = []
    identity = controller.joinpath('stat').read_text().rsplit(')', 1)[1].split()[19]
    deadline = time.monotonic() + a.timeout
    relations = []
    while time.monotonic() < deadline:
        try:
            if controller.joinpath('stat').read_text().rsplit(')', 1)[1].split()[19] != identity:
                break
        except OSError:
            break
        procs = {}
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit():
                continue
            try:
                fields = proc.joinpath('stat').read_text().rsplit(')', 1)[1].split()
                procs[int(proc.name)] = (int(fields[1]), proc, fields)
            except (OSError, ValueError):
                continue
        depths = {a.controller: 0}
        for _ in range(32):
            for pid, (parent, _, _) in procs.items():
                if parent in depths and pid not in depths:
                    depths[pid] = depths[parent] + 1
        rows = []
        for pid in depths:
            try:
                parent, proc, fields = procs[pid]
                executable = os.readlink(proc / 'exe')
                name = Path(executable).name
                if name not in {'sudo', 'lurker', 'lurker-helper', 'cryptsetup'}:
                    continue
                # Token inspection is used only for this boolean, never persisted.
                tokens = proc.joinpath('cmdline').read_bytes().split(b'\0')
                lease = b'loop-lease' in tokens and helper_token in tokens
                row = dict(pid=pid, ppid=parent, depth=depths[pid], state=fields[0],
                           pgrp=int(fields[2]), session=int(fields[3]), tty_nr=int(fields[4]),
                           tpgid=int(fields[5]), executable=executable, loop_lease_role=lease)
                rows.append(row)
                if lease and name == 'sudo' and parent in procs:
                    _, parent_proc, parent_fields = procs[parent]
                    if Path(os.readlink(parent_proc / 'exe')).name == 'lurker':
                        relations.append(dict(pid=pid, parent=parent,
                            same_session=fields[3] == parent_fields[3],
                            separate_group=fields[2] != parent_fields[2],
                            background_group=fields[2] != fields[5],
                            state=fields[0]))
            except (OSError, KeyError, ValueError):
                continue
        if rows:
            samples.append(dict(at=time.time(), processes=rows))
        time.sleep(.1)
    stopped = [r for sample in samples for r in sample['processes'] if r['state'] in {'T', 't'}]
    passed = bool(relations) and not stopped and all(
        r['same_session'] and r['separate_group'] and r['background_group'] for r in relations)
    result = dict(status='passed' if passed else 'failed', controller=a.controller,
                  helper_sha256=hashlib.file_digest(helper.open('rb'), 'sha256').hexdigest(),
                  observer_sha256=hashlib.file_digest(Path(__file__).open('rb'), 'sha256').hexdigest(),
                  sample_count=len(samples), lease_relation_samples=relations, stopped_samples=stopped,
                  scope='Owned controller descendants; no argv, credentials or payload tracing')
    for name, data in [('samples.json', samples), ('summary.json', result)]:
        path = a.root / name
        path.write_text(json.dumps(data, indent=2) + '\n')
        path.chmod(0o600)
    print(json.dumps({k: v for k, v in result.items() if k != 'lease_relation_samples'}))
    return int(not passed)


if __name__ == '__main__':
    raise SystemExit(main())
