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
same hash-checked candidate CLI and writes `result.json`. It rechecks the
staged package manifest and files before reopening, then confirms normal close
removed the mount. Preserve the separate
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

OpenBSD `8.0` softraid CRYPTO/AES reached a concrete recovery limit at the same
`76f05fb` candidate. Its acknowledged workload used a 128 MiB FFS container;
archive SHA-256 was
`ce769a91ab5001608f4ddf48f80571325f3a3c175daba62ebf2ce756007cb30a`.
After the abrupt cut and normal guest boot, writable CLI reopen failed with
`filesystem must be mounted read-only; you may need to run fsck`. The suite
performed no repair, force mount or writable substitute for unsupported read-only
crypto. Acknowledged data could not be inspected, so this result establishes a
blocked normal recovery path without establishing data loss. The full image,
external readiness and redacted diagnostic remain under
`/mnt/virt/lurker-fault-b202611142`.

| Native AES case at `76f05fb` | Recovery result |
| --- | --- |
| Arch Linux LUKS2 / Btrfs | Passed 5 checks |
| FreeBSD GELI / UFS | Passed 5 checks |
| NetBSD CGD / FFSv2 | Passed 5 checks |
| OpenBSD softraid / FFS | Writable reopen refused; filesystem check required |
| DragonFly LUKS1 | Poweroff untested: original IDE `hda` and UEFI need a reviewed private-firmware adapter |

The final audit found all ten original guests running with their expected source
paths. The four copied source definitions still matched their saved baselines;
no original block jobs remained. All owned clones, isolated networks and metadata
probe pools were stopped, while all four private disks and evidence were retained.
The concise aggregate and full host audit are ignored artifacts at
`target/vm-powerloss/summary.json` and `target/vm-powerloss/final-host-audit.json`.
These are three passed cases and 15 recovery checks, one refused recovery, and
one untested firmware configuration. They retain their exact `76f05fb` runtime
attribution and do not qualify later candidates or untested backends/ciphers.

## Reviewed DragonFly IDE/UEFI clone adapter

The DragonFly adapter preserves its reviewed IDE `hda` disk target and read-only
OVMF loader. It replaces the original NVRAM path with a fresh `nvram.fd` under
the owned run directory, initialized by libvirt from the administrator-controlled
OVMF variables template. It never reads, copies or modifies the original VM's
variables file. The loader/template paths, every ancestor's ownership/mode and
both file digests are checked; arbitrary firmware and other UEFI layouts remain
rejected. The private NVRAM destination must initially be absent, including no
dangling symlink. Once created, its device/inode is recorded and rechecked for
boot, poweroff and console/SSH identity verification. It remains with the private
disk as evidence after the transient clone stops.

Libvirt documents fresh template initialization and retained transient NVRAM:
https://libvirt.org/formatdomain.html#bios-bootloader

For a DragonFly run with separate fixture storage, pass
`--dragonfly-fixture-disk` to `prepare` and run `provision-fixture-disk` on
the prepared directory before boot. The new 1 GiB qcow2 is empty and has
a recorded inode. The clone has exactly its IDE root and the VirtIO disk
with serial `LURKERFAULTDATA`. Boot and poweroff require both identities.
Inside the clone, confirm the device against that topology before formatting
it. Mount the new filesystem at the guest's fixture base and record both the
root and fixture filesystem options. Keep the original source and previous
fault disks intact.

## NetBSD CGD repeat test on the 2026-11-20 candidate

The hash-verified `2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1` package
(`91b001fd98ddbcd9df240c8dcccfa814d51864b4d1c7a5982c955d9acf175d41`)
created and opened a 128 MiB CGD/FFSv2 container on an isolated NetBSD 11.1
clone backed by persistent FFS. The worker acknowledged file fsync, rename
and directory fsync, and a SQLite FULL/WAL commit with the connection held
open. The live worker was still present at the guarded KVM power cut, about
25 seconds after readiness.

The package and CLI hashes were verified after reboot. Normal writable CLI
reopen succeeded, but the mounted filesystem was empty. The acknowledged
file and SQLite DB/WAL were absent, so this attempt failed durability
verification. The clone's network needed the same static address reapplied
at its console after reboot. No test-container repair or forced detach ran.
The earlier passing case at `76f05fb` remains a separate observation.
Package deployment, readiness, KVM events and the failed result remain
under `/mnt/virt/lurker-fault-nb20261120a/`.

Writable CGD mounts at `24dcc6e7ff7f4bd1318ccbc238a9616cd7c791dd` had active
WAPBL journaling and the guest's WAPBL disk-cache flush setting was enabled.
The repeat cut about 0.9 seconds after readiness preserved the fsynced
65,536-byte file and its renamed directory entry. The raw SQLite WAL recovered
at 32 bytes instead of the acknowledged 82,432 bytes. The database reopened
without the committed table. This is a partial recovery and a second failed
durability attempt. See `result-nb20261120log.json` in the same evidence directory.

On NetBSD 11, ordinary application `fsync` requests `FSYNC_WAIT`. Plain FFS
requests an underlying cache flush in its explicit `FSYNC_CACHE` path. FFS
with WAPBL may skip a journal flush for a vnode without dirty metadata buffers,
and a WAPBL journal commit can request a cache flush through CGD and vnd.
`vndconfig -i` selects the vnode file-I/O path that the kernel already selects
for sparse images. Its writes use `IO_DIRECT` and have no explicit `IO_SYNC`
flag. These paths do not establish that the tested SQLite acknowledgment
reached the encrypted backing file before power loss.

For a separate diagnostic on a new NetBSD CGD fault fixture, add
`--netbsd-sync-diagnostic` to `guest-setup`. It remounts the newly opened
fixture in synchronous journaled mode before the worker starts and records the
effective flags in the result. This option applies only to the test fixture.

The synchronous journaled diagnostic also recovered the acknowledged file
but had a 32-byte WAL and no committed table after the cut. The effective
mount flags showed both modes on the same filesystem. The raw post-reboot
SQLite evidence and `result-nb20261120sync.json` are retained. Synchronous
mounting did not recover the transaction in this test.

An isolated `a60e4bc` repeat kept the normal WAPBL mount and selected
read-cache-only mode on the guest's backing VirtIO disk with
`dkctl ld0 setcache r`. The guest reported its read cache enabled and
omitted write-back from `getcache`. The fsynced file recovered with its
original digest. The SQLite WAL recovered at 32 bytes and the committed
table was absent. Disabling virtual disk write-back alone did not recover
the transaction. See `result-nbwt20261120.json` under
`/mnt/virt/lurker-fault-nb20261120a/`.

https://man.netbsd.org/NetBSD-11.x-BRANCH/dkctl.8

On the original NetBSD 11.1 guest, a disposable `a60e4bc` CGD mount with
WAPBL active was used to trace SQLite 3.53.4. An ordinary FULL/WAL commit
and a repeat with SQLite's `fullfsync` pragma enabled produced ten
`fsync` calls each from transaction setup through normal shutdown, with
zero `fsync_range` calls. Both left 82,432 bytes of WAL before close.
An explicit `fsync_range(FFILESYNC | FDISKSYNC)` on the WAL succeeded for
whole-file and nonempty-range requests. In the examined NetBSD 11 source,
both journaled FFS paths return before the explicit `FSYNC_CACHE`
device-flush branch. The whole-file path flushes WAPBL when the vnode has
dirty metadata buffers. The nonempty-range path calls `wapbl_flush` when
eligible, regardless of the vnode's dirty buffer list.

The owned CGD mapping advertised write-back without a changeable cache
mode or FUA support, and its `setcache r` request returned ENOTTY.
`dkctl cgd0 synccache force` on that same fixture returned success
through `DIOCCACHESYNC` while the SQLite connection remained open. CGD
forwards this explicit request to vnd, which asks the backing filesystem
to sync the ciphertext vnode with `FSYNC_DATAONLY | FSYNC_WAIT |
FSYNC_CACHE`. This verifies an opt-in device ioctl on the tested mount.
The explicit request requires privileged raw-device access. The fixture
closed normally, all CGD and vnd devices were idle, and the original guest
configuration was untouched. Evidence is under `target/netbsd-cgd-barrier/`.

On the isolated clone, the exact `a60e4bc` package and a fresh WAPBL fixture
repeated the workload. `dkctl cgd0 synccache force` returned success with
82,432 bytes of WAL and the workload process still alive. The cut began
1.17 seconds after the ioctl returned. Normal CLI reopen recovered the
acknowledged file digest. The raw WAL had 32 bytes and the committed SQLite
table was absent. Normal close detached the mount. The explicit device
flush left this WAL durability failure unresolved. See
`synccache-nbcgdflush20261120.json`, `result-nbcgdflush20261120.json`
and `synccache-recovery-and-close.log` under the NetBSD fault directory.

The `--netbsd-allocated-diagnostic` control gave the same result on
`a60e4bc`. Before mount the harness replaced the new 128 MiB sparse
fixture with a byte-identical, fully allocated copy and kept the sparse
inode as evidence. Allocation rose from 425,984 to 134,299,648 bytes
without changing the CGD parameters. Independent native trace evidence
in `target/netbsd-cgd-geometry/` shows that this allocation switches
vnd from vnode read/write to direct strategy I/O.

After the cut, the normal WAPBL reopen recovered the file digest but
the WAL was 32 bytes and the committed table was absent. Normal close
detached the fixture. See `result-nballocated20261120.json` and
`allocated-cgd-recovery-and-close.log` under the NetBSD fault directory.
The control found no basis to make full allocation the CGD default.

https://github.com/NetBSD/src/blob/9c8db5d51941ff57f90f49dff82ce0edf15b3f0f/sys/ufs/ffs/ffs_vnops.c

https://github.com/NetBSD/src/blob/9c8db5d51941ff57f90f49dff82ce0edf15b3f0f/sys/dev/cgd.c

https://github.com/NetBSD/src/blob/9c8db5d51941ff57f90f49dff82ce0edf15b3f0f/sys/dev/vnd.c

On the original NetBSD 11.1 guest, the `a60e4bc` CLI mounted and closed a
128 MiB sparse CGD fixture and a byte-identical, fully allocated copy with
the same parameters. Both mounts had WAPBL active and reported 512-byte vnd
sectors. The outer FFS device also reported 512-byte sectors. DTrace
recorded 50 calls from the vnd worker to `vn_rdwr` during the sparse
reopen and close. During the fully allocated copy's mount and close, it
recorded 50 direct calls to `VOP_STRATEGY` from the vnd worker and zero
calls to `vn_rdwr` from that worker. The NetBSD 11 source selects vnode
read/write for a sparse backing file and permits the direct strategy path
when the backing vnode and device geometry support it. Full allocation
changed the live I/O path in this fixture without changing the container
format or mount flags. This comparison used clean closes. WAL durability
through an abrupt cut on the fully allocated path remains unverified.
See `target/netbsd-cgd-geometry/trace.log` and `backing-and-cleanup.log`.

On the same outer FFS, `posix_fallocate` returned `EOPNOTSUPP` for a new
file and an existing sparse 128 MiB file. The call left both allocations
unchanged and preserved the bytes already present. The fully allocated
copy above used sequential writes. See `target/netbsd-cgd-geometry/posix-fallocate.log`.

## FreeBSD, OpenBSD and DragonFly repeat tests

| KVM clone and source | Result after the confirmed power cut |
| --- | --- |
| FreeBSD GELI/UFS at `029cf99` | Writable reopen succeeded. The 65,536-byte file had a different digest. The 82,432-byte WAL was visible, but SQLite on copies of the recovered files found no committed table. |
| OpenBSD RAID1C/FFS at `2ebae8f` | Both members were supplied for the native reopen. FFS refused writable mount and required a filesystem check. Acknowledged data could not be inspected. |
| DragonFly LUKS1/UFS at `2ebae8f` | The clone's outer UFS root failed its normal boot check and stopped at a single-user prompt before networking. Encrypted volume recovery could not be tested. |

The cut followed acknowledgment by 0.45 seconds on FreeBSD and 2.07 seconds
on OpenBSD. All three guests used isolated disk copies and kept the live
workload at the cut. No manual repair was performed. The recorded package
identities, result files and hypervisor events are under
`/mnt/virt/lurker-fault-fb20261120a/`, `/mnt/virt/lurker-fault-ob20261120a/`
and `/mnt/virt/lurker-fault-dfwin20261120/`.

### OpenBSD RAID1C recovery on copied members

After the `2ebae8f` RAID1C writable-reopen failure, two hash-verified copies of its cut
ciphertext members were attached by the native tools. Both member roles and
the array UUID matched the recorded descriptor. The inner FFS mounted
read-only and recovered the exact fsynced file and committed SQLite row.
SQLite integrity passed and the uncommitted row was absent.

Native assembly used writable ciphertext copies and changed them. Both
original member hashes were unchanged. The read-only inner filesystem
unmounted normally and the vnode and softraid inventories matched their
pretest state. No repair ran. The normal CLI writable-reopen failure remains
open. See `raid1c-copied-ffs-readonly-dispatch.log` in the OpenBSD fault
directory.

### OpenBSD softraid CRYPTO repeat and copied-image recovery

The coordinated package with the VeraCrypt barrier at `4189679` also ran
a fresh native softraid CRYPTO/AES cut on OpenBSD 8.0. The cut began 2.13
seconds after the worker acknowledged the file fsync and committed
SQLite FULL/WAL transaction. The normal CLI reached inner FFS. Writable
mount was refused with the same filesystem-check diagnostic as at
`76f05fb`.

A separate native attachment of a SHA-256-matched copy allowed a read-only
FFS mount without repair. The fsynced file and committed SQLite row matched,
integrity passed, and the uncommitted row was absent. Native assembly
modified the writable copy and preserved the original ciphertext hash.
Normal dismount restored both the vnode and softraid inventories. See
`result-obcrypto20261120.json`, `obcrypto-reopen.log` and
`crypto-copied-ffs-readonly-dispatch.log` in the OpenBSD fault directory.

### DragonFly with separate fixture storage

At `a60e4bc`, the cloned guest used a synchronous UFS boot filesystem and a
separate HAMMER2 fixture disk. A clean configuration boot verified both
mounts. The real LUKS1 AES vault used its normal inner UFS. The workload
acknowledged file and directory fsync and SQLite FULL/WAL before the cut.

The boot filesystem again stopped at the UFS check. A second isolated guest
booted from a healthy image and mounted a SHA-256-matched copy of the cut
fixture disk. The published vault filename and on-guest readiness receipt
were missing. A surviving sparse staging image contained the LUKS1 magic
and rejected the recorded passphrase during a read-only unlock. The test
therefore failed to recover the published vault and could not inspect its
inner UFS. Both original cut disks were preserved. Evidence is under
`/mnt/virt/lurker-fault-dffault20261120b/` and
`/mnt/virt/lurker-fault-dfrecover20261120/`.

### DragonFly with a vault published before a clean reboot

The same `a60e4bc` package created a LUKS1 AES vault on a separate HAMMER2
fixture disk. A clean shutdown and a new guest boot proved the published
container survived with identical ciphertext. The normal CLI then unlocked
and mounted its inner UFS. The clone was cut 1.01 seconds after the host
observed the file fsync and SQLite FULL/WAL acknowledgement. The workload was
still running at the cut.

A healthy, isolated DragonFly guest mounted a byte-identical copy of the cut
HAMMER2 disk. The published vault survived and its 128 MiB ciphertext still
matched the digest from before the workload. The on-guest readiness receipt
was missing. The recovery harness used the host's saved receipt on the copy
only. The normal CLI unlocked and mounted the vault, but the fsynced file and
all three SQLite files were absent. Normal close detached the mapping and
vnode. No filesystem repair ran, and the original cut image kept its SHA-256.

This establishes loss of the acknowledged writes for an established vault
in this HAMMER2-backed configuration. It does not isolate the component that
failed to persist the writes. The cut and recovery guests, receipts, and
device inventory are recorded under
`/mnt/virt/lurker-fault-dfexisting20261120/` and
`/mnt/virt/lurker-fault-dfreadexist20261120/`.

Another established vault used the same isolated configuration. After the
application acknowledgement, a separate process opened the identified backing
file and successfully called `fsync` on its descriptor. The clone was cut
with the workload still running, 1.32 seconds after the host observed
readiness. On a verified copy, the vault again unlocked and mounted. Its
ciphertext matched the pre-workload digest, the acknowledged file and SQLite
files were absent, and normal close restored the mapping and vnode inventory.
The original cut disk was preserved. This control records failed durability
even with an explicit backing-file fsync in this HAMMER2 configuration.

The reference DragonFly 6.4.2 vnode disk sends an incoming flush to
`VOP_FSYNC` on the backing file. HAMMER2's file fsync implementation
documents a separate filesystem sync for propagating inode state to the
volume root. This source inspection leaves the exact I/O sequence in the
running guest unverified. Evidence is under
`/mnt/virt/lurker-fault-dffsync20261120/` and
`/mnt/virt/lurker-fault-dfreadfsync20261120/`.

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/dev/disk/vn/vn.c

https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/hammer2/hammer2_vnops.c

### DragonFly with synchronous UFS backing storage

A repeat used the `a60e4bc` archive and replaced the separate HAMMER2
fixture disk with synchronous UFS. The vault was published and verified
across a clean reboot before the same LUKS1 AES file and FULL/WAL workload.
The kernel reported a synchronous backing UFS mount. The guest was cut
1.32 seconds after the host observed readiness while the worker was alive.

The outer UFS refused writable mounting after the cut. Native preen and a
following read-only check succeeded on an independently hash-verified
copy. Its on-guest readiness receipt matched the host's. The 128 MiB
ciphertext matched its pre-workload digest, and the normal CLI mounted an
empty inner UFS. The acknowledged file and all SQLite files were absent.
Normal close restored the vnode and mapper inventories, and the original
cut image retained its hash. This reproduces the loss with synchronous UFS
as the backing filesystem. Evidence is under
`/mnt/virt/lurker-fault-dfouterufs20261120/` and
`/mnt/virt/lurker-fault-dfreadufs20261120/`.

### OpenBSD VeraCrypt on the coordinated package

The `a60e4bc` OpenBSD VeraCrypt AES package successfully reopened the
encrypted image after an isolated cut. The mounted inner FFS lacked the
acknowledged file, and none of the SQLite database, WAL or SHM files
was available to copy. Normal close succeeded after collection.
Evidence is in `result-obvc20261120.json` under the OpenBSD fault directory.

With the ciphertext-write barrier in
`4189679af84777daf222033ceaa5c0317b79291b`, another cut followed workload
acknowledgment by 2.07 seconds. The normal CLI reopen reached FFS, which
required a read-only mount and suggested a filesystem check. The failed
reopen left no mounted inner filesystem or attached vnode disk.

For diagnosis, the exact private engine attached a SHA-256-verified disposable
copy of the ciphertext and the inner FFS was mounted read-only. The fsynced
file matched its recorded digest. SQLite on separate working copies of the
recovered database and WAL reported integrity OK, returned the committed
row intact, and excluded the uncommitted row. Normal unmount and dismount
restored the original device inventory. Both ciphertext hashes were unchanged
and no repair ran. This verified recovery of the acknowledged data on the
tested copy. The regular CLI writable-reopen check still failed. The diagnostic
used a writable crypto attachment to the copy and a read-only FFS mount.

The staged archive SHA-256 was
`52829c0d1b9b3d7f4d96309882d634d51fc2b96a745dad9f0ee9f2a6711d8b68`.
See `result-obvcfix20261120.json`, `reopen-vcflush.log`, and
`copied-ffs-readonly-dispatch.log` under
`/mnt/virt/lurker-fault-ob20261120a/`.

### OpenBSD automatic preen on disposable copies

Fresh ciphertext copies from the three OpenBSD cut fixtures were attached
and identified as FFS2. A forced read-only check of each unmounted raw
partition returned zero and printed inconsistencies, including a duplicate
block and differences in allocation summaries. The subsequent native
preen check returned exit 8 for duplicate blocks on all three cases.
The experiment stopped at preen and normally detached each copy. It
preserved the original ciphertext hashes and device inventories. The
read-only checks' exit codes alone did not establish filesystem consistency.

See `copy-preen-superblock-audit.log` and `copied-ffs-preen-*.json` in the
OpenBSD fault directory. The separate read-only FFS inspections above
recovered the acknowledged data without mutating the inner file systems.

https://man.openbsd.org/OpenBSD-8.0/fsck_ffs.8

### OpenBSD synchronous FFS after a fresh cut

With the same `4189679` package, `--openbsd-sync-diagnostic` remounted each
fresh VeraCrypt AES, softraid CRYPTO and RAID1C FFS2 fixture with
`mount -u -o sync` before the measured workload. The effective mount table
confirmed `synchronous` on the same fixture device. Cuts began 1.43 to 2.06
seconds after readiness. Direct normal writable reopen still failed at FFS
on all three routes.

A separate native `fsck_ffs -fp` on each hash-verified ciphertext copy
repaired its allocation summaries and marked FFS clean. The following
read-only filesystem check exited zero on every copy. After ordinary
native detach, the normal Lurker CLI opened each repaired copy writable,
recovered the exact fsynced file and committed SQLite row, excluded the
uncommitted row, synced a new file and closed normally. The original
ciphertext hashes and device inventories matched the baseline. These
controls qualified copied-image recovery through explicit repair. They
did not add filesystem repair to Lurker's mount path.

See `synchronous-copied-ffs-audit.log`,
`copied-ffs-preen-*-sync.json` and `result-*sync20261120.json` in the
OpenBSD fault directory.

The production `ca3c845` archive repeated all three cuts with the package's
default mount policy. The kernel reported `synchronous` on every workload
mount, without a diagnostic remount. The cuts began 1.91 to 2.52 seconds
after readiness. FFS required a manual check before writable reopen on all
three routes. Native preen succeeded on each separate verified ciphertext
copy, followed by a clean read-only check, ordinary writable CLI reopen,
recovery of the fsynced file and committed SQLite row, exclusion of the
uncommitted row, a new fsynced write and normal close. The original fixture
hashes and the clone's mount and device inventories were preserved.

Archive SHA-256 was
`22d42cd27e6f2e41d9a8e619ce03ebbfe54813b387792d3e589958832e657310`.
See `default-sync-powerloss-summary.json` in the OpenBSD fault directory.

https://man.openbsd.org/OpenBSD-8.0/mount.8

## Arch packaged runtime power cuts

Packaged LUKS2/AES at `2ebae8fb78ddc821cf73b1feb3e66ade72b88ee1` passed
all five recovery checks after an actual KVM cut. The synchronized file
and SQLite FULL/WAL commit recovered, the uncommitted row stayed invisible,
and normal close detached the filesystem. The isolated guest had Btrfs
outside and inside the encrypted image. The package SHA-256 was
`3781b6dd12b2487f0a584ed7e5e1c4077cdac2be6620ff484c0755c2f29174d0`.
Evidence is under `/mnt/virt/lurker-fault-ar20261120a/`.

The same package passed all five checks per route for LUKS1/AES, VeraCrypt
AES via kernel mappings, and gocryptfs. VeraCrypt AES via its login-user LKL
transport reopened an empty filesystem after acknowledging file and directory
fsync and a SQLite FULL/WAL commit. The old LKL block device advertised no
flush support and rejected the two-buffer form of a flush request before its
host I/O callback. The failing fixture remains in the same evidence directory.

Cryptomator passed all five checks. CryFS refused to reopen the vault on the
first attempt and on retry. The private engine reported a parse failure in
`LocalStateMetadata` and exited before mounting. Its ciphertext and
integrity state were retained, along with the result and redacted diagnostic.

A focused CryFS package built on Debian at `0b412d2` passed all five power-cut
checks on the isolated Arch guest. It used a fresh persistent local state
directory, preserving the malformed state from the earlier case. The rebuilt
CryFS engine replaced local integrity metadata atomically and synchronized it
before acknowledging writes. The fsynced file and SQLite FULL/WAL transaction
both recovered. Package, readiness, result and local state path are recorded
in the Arch evidence directory. The normal compatibility check reopened
ciphertext from `2ebae8f` with byte-identical existing client metadata.

The rebuilt `390f468` Arch package passed all five checks for VeraCrypt
AES in the login-user route. Its LKL device advertises flush to Btrfs and
forwards the request to the host backing file through the private VeraCrypt
engine. The recovered file matched its digest, SQLite recovered the
committed row and kept the uncommitted row invisible, and normal close
detached the mount. Archive SHA-256
`ceb81faa6e49f22a3b4b717b3136cc7781caa410ef8a1495f609f894a3ce8c3a`;
see `result-ar20261120lklfix.json` in the Arch evidence directory.

The `1e04b85` build updated LKL to the Linux `6.12.118` stable base. Its
VeraCrypt-only archive and full Arch archive both passed all five login-user
AES recovery checks after separate cuts. The full archive recovered the
fsynced file and committed SQLite FULL/WAL row, excluded the uncommitted
row and closed normally with no remaining FUSE mounts or private drivers.

The first cut reverted the login runner's staged file permissions. Restoring
the root-owned harness permissions allowed verification without altering the
vault. The full archive repeat persisted those permissions before the workload
and recovered through the ordinary runner. The two archives used byte-identical
private VeraCrypt and LKL executables. The full archive SHA-256 was
`55bec5eed39951786cfc6f693d1aea40b0809afa27a8b80f71b443458a2267ce`.
See `result-arvcstable20261120.json`, `result-arvcfull20261120.json` and
`lkl-stable-full-cut.log` under `/mnt/virt/lurker-fault-ar20261120a/`.

### FreeBSD GELI flush tracing

On the original FreeBSD 15.1-RELEASE-p6 KVM guest, DTrace observed no GEOM
`BIO_FLUSH` during the same file fsync, rename, directory fsync and SQLite
FULL/WAL workload on fresh, disposable `029cf99` GELI volumes. This held
for both the ordinary UFS Soft Updates mount and a per-fixture remount whose
effective flags included `synchronous`. A separate `DIOCGFLUSH` call on an
owned writable raw vnode md device succeeded and produced the expected
trace event. The trace fixtures closed normally, the DTrace modules were
restored to the original state, and a fresh ordinary GELI AES Auto lifecycle
passed 13 checks, including close and reopen.

The kernel source corresponding to the tested release shows that vnode md
already defaults to `IO_SYNC` writes and that GELI forwards `BIO_FLUSH`
to md when it receives one. Vnode md then fsyncs the outer file. The
tested outer UFS fsync waits for vnode I/O without issuing a physical
provider flush. QEMU `cache=none` bypasses the host page cache and still
advertises guest writeback. The original guest reported
`dev.vtblk.0.writecache_mode=1`. These observations identify a missing cache
barrier along this tested path. They do not locate which layer lost the
bytes in the power cut. None of the native Lurker mount options tested
made ordinary applications' fsync issue that barrier. The GELI lifecycle
continues to use the native stack, with abrupt-poweroff durability unqualified.

Guest evidence is under `/tmp/lurker-freebsd-geli-flush-trace.m8KH73/`.
Both DTrace logs and the ordinary CLI summary are collected under
`target/freebsd-final-resume/ufs-source/`.

An isolated `a60e4bc` repeat set the clone's only VirtIO disk to write-through
with `dev.vtblk.0.writecache_mode=0` before the measured workload and verified
the effective value. After the cut, GELI reopened and the 65,536-byte recovered
file failed its digest check. SQLite on copies of the 4,096-byte database and
82,432-byte WAL reported integrity OK and had no committed table. The vault
closed normally after collection. This control shows that disabling this
virtual disk's cache alone did not repair the acknowledged-write failure.
Evidence is under `/mnt/virt/lurker-fault-fb20261120a/`.

https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.0/sys/dev/virtio/block/virtio_blk.c

A separate live `a60e4bc` fixture checked whether a privileged caller could
send `DIOCGFLUSH` to the mounted GELI provider itself. The device opened
read-only, but the ioctl returned EPERM. Opening the mounted provider
read-write also returned EPERM. FreeBSD's GEOM cache-flush BIO requires
write access to the provider. No power cut was triggered for this control.
The owned fixture closed normally. See `mounted-geli-explicit-flush-access.json`
under the FreeBSD fault directory.

https://raw.githubusercontent.com/freebsd/freebsd-src/releng/15.0/sys/sys/disk.h

The `--freebsd-suj-diagnostic` control enabled native soft-updates
journaling on a fresh unmounted GELI/UFS fixture from `a60e4bc`, verified
it using `tunefs -p`, and opened it through the normal CLI. Initial
setup attempts used the wrong backing sector size and the wrong status
label. Both stopped before poweroff and left no device attached. The
measured cut began 0.52 seconds after acknowledgment.

The normal writable reopen refused the dirty UFS journal. A SHA-verified
copy recovered its journal with `fsck_ffs -p`, and normal writable CLI
reopen and close then passed. The file digest differed and SQLite could
not find the committed table even though the raw WAL measured 82,432
bytes. The original ciphertext hash and device inventory were unchanged.
This control leaves GELI acknowledged-write durability unresolved.
See `result-fbgelisujc20261120.json`, `geli-suj-copy-recovery.json` and
`geli-suj-copy-fsck.log` under the FreeBSD fault directory.

https://man.freebsd.org/cgi/man.cgi?query=tunefs&sektion=8

FreeBSD gocryptfs at `a60e4bc` passed all five recovery checks after
explicit session reconciliation. Direct reopen first refused the recorded
session. Normal close quarantined the dead session and reported that its
backend exit receipt was unavailable. The next reopen restored the fsynced
file and committed SQLite WAL row, excluded the uncommitted row, and
closed normally. The guest also lost test package access-mode changes,
which were restored outside the ciphertext fixture before the verification
retry. The original failure and recovery results are preserved separately
under `/mnt/virt/lurker-fault-fb20261120a/`.

FreeBSD PEFS at `a60e4bc` also passed all five recovery checks after a cut.
The first setup stopped before poweroff because the workload tried to stat
a PEFS internal name returned by directory listing. The corrected workload
records sizes only for its own file, database, WAL and SHM. The initial
fixture closed normally, and the measured cut used a fresh fixture. See
`pefs-workload-start.log` and `result-fbpefs20261120b.json` in the same
FreeBSD evidence directory.

FreeBSD VeraCrypt AES at `a60e4bc` reopened after the cut, but the mounted
UFS lacked the acknowledged file and all SQLite files. The vault closed
normally after collecting the empty recovery inventory. See
`result-fbvc20261120.json` in the same FreeBSD evidence directory.

FreeBSD VeraCrypt AES at `2fda5f7ca413f73ac7c9d3ec51feac007119386e`
passed all five recovery checks after a fresh cut on the same isolated
FreeBSD 15.1 KVM clone, with its default VirtIO write-back setting.
The cut began 0.41 seconds after the worker recorded readiness.
The normal CLI reopened the UFS volume, recovered the exact fsynced
file and committed SQLite row, reported database integrity OK, and
excluded the uncommitted row. Normal CLI close removed the mount.
The tested archive SHA-256 is
`be8d7cf4aa3c5cc5dbb20755f2702d7df8c0c597332716411bf12ab07e81bb3f`.
See `result-fbvcflush20261120.json` and the associated readiness and
hypervisor events in the same evidence directory.

On the original FreeBSD 15.1-RELEASE-p6 guest, DTrace tied 43
`fuse_vnop_write` calls with `IO_SYNC` to the disposable VeraCrypt vnode md
thread. The identified private engine descriptor recorded 45 ciphertext
`pwrite` calls and zero ciphertext sync calls. An explicit auxiliary-image
fsync reached `fuse_vnop_fsync` with no ciphertext sync. The baseline
observer inherited ignored SIGINT. Its exact trace process terminated on
SIGTERM, then the fixture closed normally and the kernel module baseline
was restored.

FreeBSD's `2fda5f7` patch completes a ciphertext flush before returning a
successful FUSE image write and forwards explicit auxiliary fsync. The
packaged engine returned successful ciphertext fsync in every measured
workload window: four during file fsync, three during rename and parent
directory fsync, 19 during SQLite FULL/WAL commit, and three during the
follow-up syncs. The explicit auxiliary fsync control returned two successful
ciphertext syncs. The file and SQLite row matched after ordinary reopen.
The portable archive SHA-256 is
`be8d7cf4aa3c5cc5dbb20755f2702d7df8c0c597332716411bf12ab07e81bb3f`.
Both trace logs and the patched phase result are under
`target/freebsd-vc-barrier/`.

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/fs/fuse/fuse_vnops.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/fs/fuse/fuse_io.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sbin/mdconfig/mdconfig.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/dev/md/md.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/geom/eli/g_eli.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/ufs/ffs/ffs_vnops.c

https://www.qemu.org/docs/master/system/invocation.html
