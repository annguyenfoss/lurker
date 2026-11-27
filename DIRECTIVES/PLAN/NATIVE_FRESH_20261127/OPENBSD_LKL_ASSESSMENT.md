# OpenBSD native LKL prerequisite — source-only assessment

A native OpenBSD port is a plausible ordinary host-tool candidate for Btrfs and
XFS. Package absence and missing in-kernel drivers do not establish that these
filesystems are impossible on OpenBSD. No LKL compilation, RAM boot, mount,
formatter build, installation, host configuration or storage change was made
for this assessment. The ordinary-route 15-pass/one-failure ledger keeps its
original artifact identities; no new filesystem capability is qualified.

LKL runs filesystem code inside a native process, using host threads/memory and
local file descriptors, with native FUSE exposing a local mount. It requires no
VM, network filesystem or replacement boot kernel. If qualified, this driver
and its formatter belong to separately installed administrator-controlled host
prerequisites, outside Lurker's private encryption bundle. OpenBSD base FUSE
remains external as well.

## Exact inputs and native observations

The current upstream reviewed pin is
`d0f76a77e8c2ca68bd7d96190b8da09040eab072` (Linux 6.12, GPL-2.0-only).
Its LKL defconfig enables both `BTRFS_FS` and `XFS_FS`, including their POSIX ACL
configuration. FreeBSD's official port supplies a mature comparison at
`86dd3afb590eccc1903611bdaa8bac87757eb80d` (4.16.g20180628), with port patches
pinned at `46cf3d9696491c61d91ecd22db11c7f7c1e62c24`. Its native results do not
qualify an OpenBSD binary or lifecycle.

The source capture pins OpenBSD's official repository at
`96a793b7c10c98cde4c650560a45a69f638a5bff`. All four captured FUSE headers match
the actual installed OpenBSD 8.0 headers byte-for-byte. The live host still
runs GENERIC.MP#14 with the recorded syspatches. Native header/library hashes,
read-only symbol inventories, installed manual pages, exact primary-source
URLs/hashes and fetched-file failures are retained under
`target/goal-validation/native-fresh-20261127/hosts/openbsd/build/lkl-source-assessment/`.
The detailed call inventory is `build/lkl-openbsd-source-assessment.txt`.
No claim is made that this repository pin is the exact source of every running
kernel or base-library object.

## Concrete adapter requirements

- **Build and ABI selection.** Current `Makefile.autoconf` recognizes FreeBSD's
  BSD object format, looks for `fuse3/fuse.h`, adds `-lfuse3`/`-lrt`, and otherwise
  selects Linux-oriented hijack/network targets. OpenBSD provides native headers
  under `/usr/include/fuse` and base `libfuse.so.4.0`. Select an explicit OpenBSD
  POSIX/BSD filesystem-only target and native API26 adapter, without callback
  casts or linking Linux headers to the base ABI. Current API35 callback shapes,
  `fuse_config`, `lseek`, `fallocate` and `copy_file_range` do not fit the native
  operation structure. The older port also needs a real operation-field audit.
  Native Clang, GNU make and flex exist; GCC, bison, GNU sed/stat and formatters
  were not found in the inspected PATH. Missing build dependencies are ordinary
  prerequisite work, not a demonstrated compiler failure.
- **Threads and timers.** Native pthread/TLS primitives exist. Upstream's
  `pthread_getattr_np` does not; `pthread_stackseg_np` is available and returns
  the stack top, requiring checked conversion to a lower bound on amd64.
  Upstream POSIX timer callbacks require `timer_create`/`SIGEV_THREAD` and related
  calls. Native headers do not provide that interface; `syscalls.master` marks
  timer syscalls unimplemented. Weak libc timer symbols are not evidence that
  they work. A native monotonic condition-variable or kqueue timer implementation
  must prove rearm, cancellation, callback completion and destruction ordering.
  Avoid forking a live multithreaded LKL instance; first controls must remain in
  the foreground with one FUSE request thread.
- **Mapping and exported UAPI.** Public `MAP_FIXED_NOREPLACE` is absent.
  `__MAP_NOREPLACE` is private, and destructive `MAP_FIXED` is inappropriate.
  A checked non-fixed hint may accept only the exact returned address, unmap
  only its own alternate successful mapping and reject collisions/errors.
  Repair upstream's wrong-address cleanup rather than copying it. Leave
  MMU/KASAN off initially; native W^X policy rejects ordinary RWX mappings.
  Linux/native stat, signal aliases, timestamps, open flags and generated UAPI
  layouts need native compile assertions; NetBSD's header overlay is a reference,
  not OpenBSD ABI evidence.
- **Callback errors and credentials.** Upstream returns raw negative Linux
  syscall errors, while native errno values differ (for example ENOSYS 78).
  Translate generated `LKL_E*` values only at the outward FUSE boundary, preserve
  success counts/offsets and raw diagnostics, and fail unknown errors closed.
  OpenBSD lacks the xattr callbacks used for Linux ACL exposure. Emulated task
  UID/GID/supplementary groups must reflect authenticated requests; merely
  running the host process as UID1300 does not set LKL credentials. Crucially,
  `fusebuf.c:123` exports `p_tid + THREAD_PID_OFFSET` and `fuse_session.c:637`
  passes it as `fuse_context.pid`: it is a thread identifier, not a process PID.
  A native thread-to-process/birth/credential binding is required, with bounded
  kernel snapshots, full group-vector validation, stale/dead/changed identity
  rejection and serialized emulated credential transitions. Directory-service
  group membership is insufficient. No such bridge was implemented here.
- **Native FUSE semantics.** The base library requires root for mount/unmount
  and the kernel always enforces default permissions. `create` and `access`
  callbacks are not used; regular creation arrives through `mknod`, and open
  omits O_CREAT/O_TRUNC. Use the native operation model and preserve caller
  ownership rather than overriding reported ownership. Actual unrelated-user
  denial, group-only access and later newly created file ownership remain gates.
- **Durability and close.** `virtio_blk.c` advertises no FLUSH feature and
  rejects two-buffer flush requests; host fsync code alone cannot establish
  that the emulated filesystem sends barriers. Explicit additive flush
  negotiation and error propagation are required. More significantly,
  `fuse_vnops.c:1730` returns success immediately for directory fsync, so a user
  directory-fsync result is not proof that LKL metadata became durable. A
  reviewed namespace-commit/flush strategy is required, with real failure
  injection; merely retaining an unused fsyncdir callback is insufficient.
  `fuse_unmount` closes its channel and can call `unmount(..., MNT_FORCE)`.
  Do not call that path while the mount exists or terminate the serving loop on
  a stock signal. Keep serving through external normal unmount, verify absence
  of the captured native fsid, then check LKL sync/read-only remount/unmount/halt,
  host backing flush and source close before acknowledging success. Upstream
  stop_lkl only prints unmount errors and does not supply this acknowledgement.
- **Raw providers.** Upstream capacity uses SEEK_END, which is not established
  for OpenBSD raw partitions. Use validated opened-node geometry/disklabel data
  and checked sector alignment/range; DIOCGPART contains kernel pointers and is
  documented in the header as internal. Never discover targets by guessing a
  descriptor file. The sd driver implements DIOCCACHESYNC and routes softraid
  cache synchronization through its discipline, but sd may suppress unsupported
  hardware flush requests. The reviewed vnd ioctl switch has no DIOCCACHESYNC
  case. Therefore neither a regular-file fsync result nor sd ioctl success
  proves all VeraCrypt/vdev backing durability. Each exact provider needs a
  verified flush chain; unsupported requests must not become false success.
- **Native Create remains independent.** Neither mkfs.xfs, mkfs.btrfs nor
  ntfs2btrfs is installed. Mature source-built xfsprogs and ntfs2btrfs candidates
  require their own OpenBSD source/ABI/geometry/flush assessment and actual native
  format controls. Foreign-preformatted images cannot qualify native Create.
  NTFS-to-Btrfs conversion of a new empty native NTFS fixture is a candidate
  creation route only after the converter and resulting format are qualified.

## Directory durability without kernel changes

A synchronous namespace-reply boundary is plausible for current LKL. At the
reviewed pin, `fs/sync.c:149` implements syncfs using `sync_filesystem` plus
`errseq_check_and_advance`; the latter can consume a previously reported error,
so the driver must also latch its first I/O/barrier error. The filesystem sync
path writes and waits for dirty inodes, invokes synchronous filesystem metadata
sync, and waits for block writeback. Btrfs's wait path commits its transaction
(`fs/btrfs/super.c:1001`); XFS's wait path forces its log synchronously
(`fs/xfs/xfs_super.c:768`). These are concrete source mechanisms, not runtime
proof that an OpenBSD provider honors them.

The candidate boundary would serialize each namespace/metadata mutation, retain
its raw result, call syncfs on a held descriptor for that same mounted emulated
filesystem, and complete a checked host backing barrier before returning success
to FUSE. Data/fsync/close and final teardown must use the same error latch.
Any emulated or host I/O error remains sticky for the session; later successful
sync calls cannot erase it or produce a clean close receipt. Successful
publication is acknowledged only after its namespace change and prerequisite
writes meet this boundary. It does not replace existing atomic rename/link
publication with partial-file visibility. Freeze operations, multiple writers
and unsupported flush paths must be excluded or qualified explicitly.

This may compensate for the host's omitted fsyncdir request without modifying
OpenBSD, at a performance cost. It still needs native order/fault tests proving
that a reply cannot precede commit/barrier, that asynchronous errors remain
observable, and that barriers reach the exact backing provider. The raw vnd
flush gap is unresolved; no generic durability claim or mount approval follows.
The proposed first source/build/boot ownership is limited to
`native/openbsd/lkl/`, an OpenBSD native producer script and its native tests.
Shared core, UI, stager and capability registrations remain out of scope until
actual qualified filesystem controls and review.

## Smallest reviewed next phase

First provide a separate work-specific native build filesystem with at least
6 GiB free; root currently has about 1.7 GiB. Preserve both existing 512 MiB
scratch devices and their failure bytes. Pin sources/notices and capture the
first unmodified native compilation diagnostics with one worker and a deadline.
Then review the host-only patches and generated UAPI layouts before a bounded
RAM-only boot test. No installed provider, Lurker capability or private backend
registration follows from a compile or boot success.

Before any filesystem mount, require native unit/probe coverage for timers,
stack bounds, collision-safe mapping, exact errno translation, authenticated
thread/request credentials, ordered flush failures and normal close. Only then
use new regular-file images for native formatter, mount/write/fsync/close and
reopen controls, actual caller/group ownership, unrelated-user denial, deliberate
nonempty-rmdir/unsupported-operation errors, busy-unmount preservation, and
injected metadata/flush/close failures. Test raw providers only after file
controls and current opened-node identity/flush proofs. Product integration and
container/device/backing matrices remain later gates.

## Subsequent reviewed library and RAM diagnostic

After the source assessment, the separately authorized build phase used a new
12 GiB qcow2, serial `lurker-ob-lkl-4e4015`, bound to the existing domain UUID.
The native guest identified its unique first-sector sentinel before base-FFS2
initialization as sd3a. The build filesystem is
`/srv/lurker-openbsd-lkl-build`; neither original 512 MiB scratch disk was
repurposed. A read-only reread of the failed sd1 image retained SHA-256
`6ed0ada081be4f84084b3e8aad4c789bcbc2056d8989168ce597c2879497dbbb`.
An ordinary enumeration reboot and the scoped new build volume are distinct
from the already completed ordinary-product reboot evidence.

The unmodified base-tool diagnostic failed in linker/Clang configuration.
A separate GNU-tool-only diagnostic reached native modpost compilation and
failed on missing byteswap.h. Both first failures remain intact. The reviewed
six-file OpenBSD host patch has SHA-256
`8d418903144152e49276e2b2b52071d31170bd7f0b29bdee926ed1bbbba63be6`.
Its timer, safe mapping and UAPI components passed native controls before the
fresh adapted tree rebuilt all host/kernel objects and exported host_ops
headers together. No stale additive-ABI objects were reused. Native GNU bc
1.08.2 was also required: base bc rejected -q, and a preserved no-q control
returned zero while emitting a parser error and bogus HZ0 constants. The
private GNU bc generated the actual HZ100 header successfully.

All compilation ran as UID1300, one worker. The first 600-second adapted
compile window ended at its deadline; the owned process group was confirmed
gone. A subsequent window verified the same 86,818 original source inputs and
unchanged adapter identities, then completed the library. The exact static
library is 328,372,428 bytes, SHA-256
`d07dbeda5039932129d5a66dd232fd8d92c5d4e0aafecdbb98f718fe0070d4d3`.
MMU/KASAN remain disabled. Host VIRTIO_NET/VIRTIO_NET_FD and Linux hijack targets
remain disabled; inherited CONFIG_NET=y is recorded honestly. No native
network transport or network-filesystem backend was built for this phase.

Actual generated-header layout assertions passed. The linked native RAM
contract then passed boot, getpid, measured nanosleep, halt and exactly one
post-CPU/tick-shutdown, pre-memory-free timer-quiescence callback. An additional
native negative control lowered only the emulated effective UID: halt returned
Linux EPERM, left the instance running and callable, and did not drain timers.
Restoring that emulated UID permitted successful halt and one drain. This
matches setup.c:145–167: a negative reboot result returns before running-state
change, CPU wait, cleanup, timer quiescence, free_mem and join. A future driver
must retain its source/state on this error and cannot acknowledge clean close.

The unchanged upstream RAM boot suite passed **35/35** as UID1300 against the
same library, executable SHA-256
`eed5f958cb95cb65f91bbfc32d0f5450ca8fa657ca3f382c01e0fb58eeab3d40`.
It exercised only emulated RAM files, proc and internal loopback. No host disk
was registered and no FUSE operation ran. The actual native root/build-FFS
mount inventories were identical before and after both controls. The static
library and generated SDK were archived and the contained library hash checked
again. The indexed receipts, exact binaries, raw logs and source anchors are
`build/lkl-native/library-ram-index.json` under the host evidence root.

These results establish a native library/ABI/timer/RAM foundation. Native
FUSE API26 adaptation, authenticated THRID-to-process request credentials,
outward errno translation, synchronous namespace/barrier error handling,
provider flush chains, safe normal close and native formatters remain separate
unqualified gates. There was no LKL FUSE mount, formatter/device experiment,
driver installation, shared product change or filesystem capability claim.

The separately reviewed phases 1 and 2 then passed five native primitive
controls as UID1300: encoded-THRID caller binding, API26/errno/open-flag audit,
RAM credentials/poison/held cleanup, private regular-FFS-FD I/O, and release
state. A sixth native actor control proved real secondary-group list/read/write
and unrelated-user denial, plus live group mutation under unchanged THRID,
process birth and wire UID/GID. It reports the supplementary-group wire timing
limit explicitly. The final five-control batch uses a cleared environment and
records source/controller/native-header/tool hashes before execution. First
compiler failures, initial successes and the final batch remain distinct.

The I/O controls inject EIO/short I/O at actual native pread/pwrite/fsync
boundaries and retain the first error after later successful operations. They
call the external disk-ops FLUSH primitive directly. The frozen library still
has no adapted virtio FLUSH negotiation, no emulated disk was registered, and
these controls do not qualify a filesystem barrier. Release tests use real
pipe descriptors and synthetic identity observations: native fsid detection,
actual FUSE interruption/normal-unmount service and real filesystem drain are
still unimplemented. No formatter, mount, provider installation, raw-device
admission or product capability follows. The detailed source/test boundary is
`native/openbsd/lkl/README.md`; receipts are indexed separately at
`build/lkl-native/primitives-index.json` under the host evidence root.

Primary references:

- https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
- https://github.com/freebsd/freebsd-ports/tree/46cf3d9696491c61d91ecd22db11c7f7c1e62c24/filesystems/lkl
- https://github.com/openbsd/src/tree/96a793b7c10c98cde4c650560a45a69f638a5bff/lib/libfuse
- https://github.com/openbsd/src/blob/96a793b7c10c98cde4c650560a45a69f638a5bff/sys/miscfs/fuse/fuse_vnops.c
- https://github.com/openbsd/src/blob/96a793b7c10c98cde4c650560a45a69f638a5bff/sys/miscfs/fuse/fusebuf.c
- https://github.com/openbsd/src/blob/96a793b7c10c98cde4c650560a45a69f638a5bff/sys/kern/syscalls.master
- https://man.openbsd.org/pthread_stackseg_np.3
- https://man.openbsd.org/mmap.2
- https://man.openbsd.org/fuse_mount.3
- https://man.openbsd.org/fuse_new.3
