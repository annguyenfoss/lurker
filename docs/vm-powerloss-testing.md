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

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sbin/mdconfig/mdconfig.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/dev/md/md.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/geom/eli/g_eli.c

https://github.com/freebsd/freebsd-src/blob/95719d8a895b/sys/ufs/ffs/ffs_vnops.c

https://www.qemu.org/docs/master/system/invocation.html
