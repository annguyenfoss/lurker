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
No hypervisor private SSH key is transferred to a guest; disk copies naturally retain existing guest keys.

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

Inside the verified clone, copy this script and `test-backends.py` beside each
other. Run `guest-setup --run RUN --cli ABSOLUTE_PACKAGE_CLI --backend BACKEND
--commit FULL_COMMIT --persistent-home-confirmed` using the native privilege
route (root for native containers, login user for directory backends). Add
`--unprivileged` only for Linux's VeraCrypt user route. Check the guest's mount
table before confirming persistent home storage; Linux tmpfs/ramfs is rejected
by the script as well. Setup starts a detached worker and returns its PID and
persistent directory. A returned PID alone is not readiness: read and retain
`ready.json`, whose status must be `ready-for-abrupt-poweroff`, before using
the coordinator's guarded `poweroff` and `boot` operations. Keep the worker
alive at the power cut. After boot, `guest-verify --run RUN` reopens through the
same hash-checked candidate CLI and writes `result.json`. Preserve the separate
hypervisor poweroff/boot evidence alongside the guest result; invoking a guest
verification command alone does not prove a power cut happened. Setup and
verification retain fixtures and never run filesystem repair.

A completed daemon-owned copy can be mode 0600 and unreadable to the coordinator.
`finish-copy-boot DIRECTORY` resumes only when the retained copy log confirms
success and the source checks still pass. For this permission case, a unique
transient `dir` pool registers only the owned run directory without `--build`.
The daemon's volume XML proves qcow2 format, positive capacity and no backing
file; the full copy completion is retained separately. The coordinator checks
pool name, UUID and target before stopping that metadata registration with
`pool-destroy`. It never uses `pool-delete`, deletes volumes, changes permissions
or modifies existing pools. `copy-volume.xml` and `probe-pool.json` retain this
inspection evidence.

If host DHCP policy prevents a lease, configure only the owned clone with the
manifest subnet's host 10 address and disable DHCP in that clone. Do not change
host firewall or primary networking. `static-transport DIRECTORY` accepts no
arbitrary address: it checks the owned domain/network, an exact direct host
route over the owned bridge, and that the address's ARP neighbor is the clone's
fresh MAC. The explicit assignment is saved in `static-transport.json`; every
SSH connection repeats route and MAC verification and retains strict original
`HostKeyAlias` trust. A changed bridge, gateway, neighbor MAC or incomplete ARP
entry fails closed. Persist the clone's static guest configuration so recovery
uses the same isolated address after reboot.

## Native characterization evidence

The first isolated proof on 2026-11-14 passed five recovery checks for packaged
Linux LUKS2/AES at candidate `76f05fbd2167313273c53201a070252719e4f4ea`.
The archive SHA-256 was
`6c4b562dc7ac9766b9373eca4c628830bb5a156e415c6c6563b9df5224bf6dfe`;
the separate PTY harness included `8bf9f0e`. Arch kernel `7.2.15-arch1-1` used
cryptsetup 2.8.8, persistent outer Btrfs and inner Btrfs. After an acknowledged
file fsync, rename/directory fsync and FULL/WAL commit with live SQLite/WAL file
descriptors, the owned clone was abruptly stopped with libvirt `destroy` and
recreated. The file digest and committed SQLite row survived; integrity passed
and an uncommitted row remained invisible. Raw database/WAL evidence was copied
before SQLite recovery. Normal CLI close succeeded; no filesystem repair ran.

The early worker's readiness file was retained externally and its verified
fixture parent directory explicitly fsynced before poweroff. This metadata-only
sync did not checkpoint the encrypted filesystem or run global `sync`; subsequent
runner versions fsync publication directories themselves. Private evidence is
under `/mnt/virt/lurker-fault-a20261114a`, including candidate/package identity,
external readiness, volume-copy proof, hypervisor events and guest result. This
single case does not qualify other ciphers, backends, filesystems or platforms.

The same five checks passed for packaged FreeBSD GELI/AES at that candidate,
using FreeBSD `15.1-RELEASE-p6`, GELI kernel/userland version 7, outer UFS and
inner UFS with soft updates. Archive SHA-256:
`0af7f8e0824d9722529a0e86f6479b5c9fd6e5dd9aad5202b343c40e3f9e2377`.
The acknowledged WAL stayed open at poweroff; normal writable CLI reopen and
close both succeeded. No test-container repair was requested. The copied guest
used its normal boot process, which announced background checks of its root
filesystem. Evidence remains under `/mnt/virt/lurker-fault-b202611140`; the clone
and its verified isolated network were cleanly stopped after collection.

Native GELI/CGD/softraid fixtures now default to 128 MiB; Linux container fixtures
use 0.35 GiB. Check the actual persistent root/home free space before installing
a package or starting a case, especially on the smaller OpenBSD root filesystem.
The guest manifest records the selected image size and exact runner/credential
transport digests separately from the immutable product candidate. Directory
backends take no size and record a null container size.

Packaged NetBSD CGD/AES also passed the same five checks at candidate `76f05fb`,
using NetBSD `11.1`, persistent outer FFS and a 128 MiB FFSv2 container. Archive
SHA-256: `0b461b383ee6367e12a656a382bd26006db0347cdc2ee0e31cea65447dbcbea2`.
The mandatory `.cgdparams` sidecar was retained and hashed before the cut.
Normal writable CLI reopen and close succeeded without test-container repair;
acknowledged data and the FULL/WAL commit survived. Exact runner and credential
transport hashes are in the guest result. Private evidence is under
`/mnt/virt/lurker-fault-b202611141`.
