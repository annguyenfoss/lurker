# Isolated KVM abrupt-poweroff tests

`scripts/test-vm-powerloss.py` is an explicit fault-suite coordinator, separate
from ordinary remote jobs. It uses `qemu:///system` on every libvirt command and
never stops, reboots, redefines, pivots or repairs a primary guest. Its source
allowlist contains the Arch Linux and four BSD primary development guests.

Prepare a concrete plan without changing libvirt state:

```sh
python scripts/test-vm-powerloss.py prepare --os archlinux --run example2026 --subnet 192.168.231.0/24
```

The new `/mnt/virt/lurker-fault-RUN` directory retains a private manifest, exact
source XML, sanitized clone XML and isolated network XML. Review these before
running `copy-boot DIRECTORY`. The preflight rejects overlapping routes/libvirt
networks, existing disk jobs, extra source media, writable firmware paths and
insufficient space for a full copy. The run directory permits traversal by
QEMU, but manifests and logs remain private. Ciphertext test fixtures and
random test credentials must also remain private and outside Git.

`copy-boot` performs a bounded full live `virsh blockcopy --wait --finish
--transient-job` at 32 MiB/s by default. `--finish` leaves the original source
active; `--pivot` and shallow copies are forbidden. It verifies the original
persistent XML, active source disk and running state afterwards and records
standalone-copy evidence before booting. It never reads an active qcow2 with
`qemu-img`. On failure, retain the run and inspect the block-copy log. Do not
abort a job unless its exact source and private destination establish ownership.
Upstream command semantics: https://www.libvirt.org/manpages/virsh.html#blockcopy

The clone has a fresh UUID/MAC and only its private copied disk. Host device
passthrough, shared filesystems, extra disks, channels, old socket/PTY paths,
graphics listeners and writable firmware are removed or rejected. Its transient
network has no forwarding or DNS; no change is made to the primary network.
Original hostnames and SSH keys are copied, so SSH requires the already trusted
original domain host key through `HostKeyAlias` with strict checking. The address
must come from the clone MAC's DHCP lease inside its verified isolated subnet.
No private SSH key is copied into a guest.

`inspect DIRECTORY`, `ssh DIRECTORY COMMAND`, `poweroff DIRECTORY` and
`boot DIRECTORY` resume from the saved manifest. Poweroff uses `virsh destroy`
after rechecking UUID, name, description, private disk and network identities.
This is abrupt KVM poweroff, not a claim of sending literal SIGKILL. Restart
recreates the saved transient definition. No command targets original guest
names for shutdown or repairs filesystems. If a clone cannot boot without
filesystem repair, stop and retain the concrete boot failure for characterization.

Run one copy/clone workload at a time. Stage the exact candidate package and
record its source revision and binary hashes. All encrypted data must live under
the guest's persistent `$HOME/.cache/lurker/fault-RUN`, with ownership and marker
checks; `/tmp` may be tmpfs. A durability workload must confirm individual file
fsync, atomic rename plus directory fsync, and SQLite `WAL`/`synchronous=FULL`
commit while keeping its connection open. Do not call global `sync` or cleanly
unmount before cutting power. Reopen through the candidate CLI and verify only
acknowledged durable data; preserve database/WAL evidence before opening SQLite.
Record interrupted writes separately. This characterizes tested operations and
never promises general corruption immunity. An unavailable guest, package or
recovery path is blocked/incomplete rather than a passing skip.

The ownership tests run without libvirt or product storage operations:

```sh
python -m unittest discover -s scripts/tests -p test_vm_powerloss.py -v
```

The coordinator alone does not qualify any backend. Retain full logs under the
private run directory and publish only concise result JSON with candidate
revision, package digest, OS/kernel/backend/filesystem, phase, poweroff method,
durable checks and any blockers. Cleanup may discard only verified owned clone
resources after evidence collection; never force unmount or repair unrelated
storage.
