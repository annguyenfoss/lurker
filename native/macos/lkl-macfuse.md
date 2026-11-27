# Native Darwin LKL prerequisite: library proof and remaining driver gates

This is a separately built host filesystem prerequisite. Keep its filesystem
code, tools and libraries outside Lurker bundles. The existing product XFS
refusal remains in place until a replacement mounted route passes qualification.
An in-process native library using local I/O is eligible; the older Btrfs
VM/network-export routes remain excluded.

## Exact source and build

- xlinuxfs: `f9da28e8cfdf6a4574a31e8baa9ba1811ef4c2e4` (2026-10-30), GPL-2.0.
  https://github.com/HuanchuanTech/xlinuxfs
- Companion LKL: `c4d0895ead9badb52f7bbffe3d529c61c8fc9485`, branch
  `darwin-fskit` (2026-10-30); Darwin host/build commits date from 2026-09-28.
  Linux COPYING and per-file SPDX licenses apply, including GPL-2.0-only for
  the host block transport and generic lklfuse.
  https://github.com/HuanchuanTech/lkl
- Build recipe: `tools/lkl/darwin/build.py` in that LKL commit. Its fixed profile
  includes ext4, XFS and Btrfs, with 714 kernel source units and matching generated
  headers. UFS is disabled. The checked-in profile avoids a Linux VM, a prepared
  sibling checkout and precompiled kernel objects.
- Buran used Xcode 27.2, managed Python 3.15.2 and upstream LLVM 19.1.7. The
  seven profile tests passed; the one-worker build produced native Mach-O
  arm64 and x86_64 archive slices. Runtime controls below used arm64 only.

The port includes Apple arm64 syscall calling-convention corrections, native
thread/condition-variable/timer operations, Mach-O section initialization and
local file descriptor I/O. Public source and native architecture are credible;
they are not grounds for declaring this route impossible. Source build and
system-mounted driver acceptance are separate gates.

## Opt-in backing flush correction

Apply `lkl-darwin-flush.patch` only to the pinned companion source. It changes
the host block transport, not the boot kernel or the filesystem implementations.
The existing `lkl_disk_add` keeps its original feature advertisement. The
additional `lkl_disk_add_with_flush` function explicitly advertises FLUSH for a
caller whose callback supports it. No existing structure or function ABI changes.
The transport also accepts FLUSH's header/status pair without requiring a data
buffer; callback failure remains a virtio I/O error.

The consuming xlinuxfs bridge control declares and calls the new function at
its existing disk registration point:

```c
extern int lkl_disk_add_with_flush(struct lkl_disk *disk);
/* Only the callback known to implement FLUSH and its errors opts in. */
int disk_id = lkl_disk_add_with_flush(&disk);
```

Its `lkl_fskit_blk_ops` callback already forwards FLUSH to
`lkfs_block_sync`, whose file backend calls native `fsync`. That function's
existence alone did not prove forwarding: the unpatched transport advertised
no FLUSH and rejected its two-buffer request. Never silently opt an unknown
custom callback into the new capability.

Private round evidence is under
`target/goal-validation/native-fresh-20261127/hosts/macos/validation/native-lkl/`.
The retained baseline archive SHA-256 is
`4db143624782ecacd6dbf8bae1e948ec36f6517343a57d3c3aa8a10800e83df3`.
The final opt-in-only patched archive is
`e7d0c4daa9bffefb0087ca68ee967a753f392c73fd0d1f8eb6ccd4dffe0bf9fe`.
Only the host `virtio_blk.o` member was rebuilt in each slice; the kernel and
other host objects came from the exact baseline. Both architecture checks passed.

Unmodified upstream platform/capability C test sources ran through a bounded
owner controller on independent image clones. Additional controls copied the
common three PDFs and text through the bridge, checked the bytes, appended text,
created another file, and verified them across four separate native processes.
The bridge reported the expected superblock/filesystem type and access mode.
Host I/O instrumentation preserved the native calls and counted their outcomes.

- Baseline ext4/Btrfs and modern XFS persisted the payload, but issued zero host
  `fsync` calls. Injected callback EIO was hidden: file fsync returned success.
  These are failed flush controls, not durability passes.
- The final patch retained all three filesystems' platform/capability and payload
  results. Host `fsync` calls were observed with successful normal returns;
  injected EIO reached direct file fsync as EIO. Normal LKL unmount then returned
  zero on the separate fault fixtures. This proves the tested forwarding/error
  path, not power-loss durability or other errno translations.
- ext4 was formatted natively on Buran. Btrfs and modern XFS seeds were imported
  from fresh Ubuntu native formatter controls with exact hashes/provenance, then
  cloned before any Mac writes. They prove native library I/O only, not native
  Mac Btrfs/XFS Create support.
- Buran's native `mkfs.xfs` 3.1.4 produced a V4 seed rejected by the newer LKL
  inode verifier. Those failed fixtures remain distinct. A later native formatter
  must be qualified before product Create can use the replacement driver.
- An earlier patched archive's combined `lipo -verify_arch` invocation had
  invalid argument syntax. Its failure is retained. Separate per-architecture
  checks passed without changing that archive; the final patch's builder uses
  separate checks from the start.

These controls expose storage only inside the native process. They are not a
host-visible mount, encryption-product matrix pass, physical-device sample or
FSKit extension acceptance. The frozen `025f558` package results remain unchanged.

## Kernel and native host symbol boundary

Apply `lkl-darwin-symbols.patch` as well as the flush patch before a new source
build. The upstream Darwin recipe made only memcpy/memset/memmove private.
Its combined kernel object also exported formatting and string functions,
including snprintf/vsnprintf, which could capture native host calls. A native
adapter review candidate crashed in that kernel formatting implementation before
kernel initialization. Avoiding one call site is insufficient.

The corrected partial link exports exactly 17 `_lkl_*` API/bridge names and four
references required by the pinned `boot_glue.S`: `___vmalloc_noprof`,
`_blake2s_compress_generic`, `_init_thread_info`, and `_jiffies_64`. All other
kernel definitions remain local inside the combined object. Native POSIX host
objects are added afterward, so their libc references resolve to Apple's
libraries. This does not change the flush patch or any filesystem source.

The initial native proof reused the exact compiled kernel and host objects from
the flush-capable archive, repeating only the kernel partial link and archive
assembly. It did not recompile filesystem sources. Both arm64 and x86_64 kernel
export sets matched the 21-name boundary, and both control executables linked.
The resulting universal archive SHA-256 is
`1902814d17f275f72da97ff4c0fcbf75cc74fe7aebab6fb3f35f3a06ed5c02f0`.
The earlier archives and their observations remain distinct.

`tests/lkl-host-libc-control.c` checks direct host function addresses and
variadic formatting before initialization, while LKL runs, and after cleanup.
Native arm64 execution bound all 18 checked names to Apple system libraries and
passed. The x86_64 executable was linked and its unresolved host symbols checked;
no x86_64 runtime claim follows. The library's three-filesystem platform,
capability, four-process payload and injected-flush-error controls all passed
again against this exact archive. These remain library controls, independently
of native formatter, installed-driver or encryption-product acceptance.

## External macFUSE driver candidate

`lklfuse-darwin.patch` adds a separate host-driver source to the pinned LKL
checkout. `scripts/build-macos-lkl-filesystem-driver.py` builds it in a fresh
private directory, verifies the explicit archive hash and both kernel export
sets, records headers/toolchain/linkage, and refuses root execution. It has no
installation or elevation mode. Filesystem code remains outside Lurker bundles;
the product XFS guard is unchanged. No replacement driver is installed yet.

Example build, with administrator-reviewed input locations and archive hash:

```sh
python scripts/build-macos-lkl-filesystem-driver.py \
  --lkl-source /path/to/pinned/lkl \
  --library /path/to/liblkl.a \
  --library-sha256 1902814d17f275f72da97ff4c0fcbf75cc74fe7aebab6fb3f35f3a06ed5c02f0 \
  --include-dir /path/to/matching/include \
  --output-dir /path/to/new/private/output
```

The candidate uses macFUSE 3's official API 35 compatibility callbacks, with
`FUSE_DARWIN_ENABLE_EXTENSIONS=0`, native arm64 linkage and compiler-checked
signatures. It does not cast Linux callbacks into Darwin extension signatures.
The generic upstream source was not a drop-in port: native compilation exposed
Darwin timestamp, open-flag and callback-ABI differences. FUSE 2 is not qualified
by this implementation. Explicit Darwin flags and Linux-to-Darwin errno mapping
remain separate; raw Linux errors are translated once at the outward boundary.

Requests serialize all emulated credential changes and restore real/effective/
saved IDs, fsuid/fsgid, groups and umask on success and failure. Creation uses the
request's uid/gid/umask; filesystem permission checks use those credentials.
Buran's native `fuse_getgroups` returns ENOSYS. Its fallback takes two complete
`KERN_PROC_PID` snapshots and requires consistent birth time, live process
state, real/saved/effective IDs, primary group and full exported group vector.
Zombie, exiting, disappeared, changed and overflowing identities fail closed.
The exported kernel advisory groups are used; account-directory membership is
never inferred. On this host that kernel vector has 16 groups, while the
account-directory `getgroups` view can have 18. Extended directory memberships
and a working native `fuse_getgroups` branch are not acceptance claims.

PID-zero/dead requests cannot perform path or data operations. Mount-root
metadata may use the mount owner; already-held fsync/flush/release capabilities
retain their opener identity so dead clients can release resources. Handles are
opaque tokens checked before raw LKL descriptors. Native controls exercised
mode/umask/ownership, setgid inheritance, a second uid's permitted operations
and denial through searchable parents, stale handles, actual errno values,
512-entry directory cookies, exclusive-rename races and duplicate backing
claims. Auxiliary direct-callback tests label synthetic request envelopes and
real native kernel/VFS operations separately from actual mounted requests.

The driver currently accepts only caller-owned, singly linked regular-file
backings with no group/other write access, opened without following symlinks,
identity-checked and exclusively locked. It refuses root and credential
mismatches. Device backings and product dispatch are not admitted. The native
mount uses local macFUSE, nodev/nosuid/noexec and nobrowse. No VM or network
filesystem participates. Imported Btrfs/XFS seeds prove driver I/O only.

Normal close requires real FUSE DESTROY, global absence of the captured native
FSID, and the original mountpoint directory identity. A native-only observer
waits for INIT without blocking it; it never calls LKL. Channel cleanup occurs
only after that gate and before draining the inner filesystem. There is no
`fuse_unmount` call: the exact macFUSE library can force an exited session's
unmount. Source review followed `fuse_destroy` through session destruction to
MFChannel closure, device-FD close/XPC cancellation; it does not request a
pathname unmount. The library's void cleanup API does not expose its channel
close result. Checked inner/backing operations provide the drain result.

The inner root descriptor is captured while healthy, retained on failed sync,
and closed after successful sync before non-forced LKL unmount. Filesystem sync,
internal unmount, device claim release, native backing fsync and final close are
checked. Per-file/directory sync errors remain sticky. Every failed native host
block request also latches an error, including background writeback: later
successful sync/unmount cannot produce a successful exit. Native INT/TERM/HUP/
PIPE dispositions and signal masks are owned explicitly after channel INIT;
signals keep service available for ordinary OS unmount and produce failure after
close. Failed drain retains its worker/backing instead of forcing release.

The final private arm64 candidate passed four mounted opens on ext4, Btrfs and
XFS with common payload write/append/verify, permission/refusal controls, busy
close and normal teardown. Exactly one native backing fsync EIO was injected
and disarmed before close on independent fixtures. ext4/Btrfs subsequently
synced and unmounted successfully but exited with failure from the permanent
host-error latch. XFS continued reporting EIO and retained its backing without
success acknowledgement. Its scoped terminal recovery is recorded separately;
it is not a normal-cleanup pass. Earlier candidate crashes, signal failures and
fault-interposer recursion are retained under their original identities.

Private receipts and exact component hashes live under
`target/goal-validation/native-fresh-20261127/hosts/macos/validation/native-lkl-adapter/`.
These do not replace the frozen `025f558` app/package results. Trusted installation
and installed-driver tests remain separate gates. Use administrator-controlled
versioned host locations and dependencies, verify copied bytes before use, and
execute as the login user. Never elevate the private build or install this code
inside the app. The FSKit restricted entitlement remains a genuine blocker for
that transport and is not bypassed by this separate macFUSE route.

Cleanup source pins for macFUSE 5.5.0: Library 3
`85f89460c4962ae94a84e25037326b22efb7f0e0`, Mount framework
`395aa1b3bb90e907c10d1cae03dcee314a2d2a8a`.

https://github.com/macfuse/library/tree/85f89460c4962ae94a84e25037326b22efb7f0e0
https://github.com/macfuse/mount/tree/395aa1b3bb90e907c10d1cae03dcee314a2d2a8a

## Native creation candidates

Assess native formatter builds independently of the imported seeds. Modern
xfsprogs needs a real Darwin portability/build assessment; the old formatter's
presence is insufficient after its rejected seed.

`ntfs2btrfs` 20260810 is a credible native Btrfs creation candidate following an
empty native `mkntfs -F -Q` fixture, as separately exercised on FreeBSD. Its
source supports non-Windows POSIX file I/O and software CRC32C on arm64. A
Darwin build must qualify C++23 modules/format/print, CMake/Ninja and compression
dependencies, and any necessary libc++ portability fixes. The non-Windows code
still notes missing device sector-size discovery; conversion/close does not by
itself prove a backing flush. Require correct size/sector identity, exclusive
access, checked synchronization, default CRC32C with data checksums, and a native
mounted close/reopen lifecycle. No converter build or Mac Create pass is claimed.

https://github.com/maharmstone/ntfs2btrfs/tree/20260810

UFS1 would additionally need a coherently regenerated LKL config/object/header
profile and filesystem probing/mount-option integration. The current profile
has no UFS objects. Linux's UFS write option is marked experimental; source
availability alone does not qualify it. This is an integration gap rather than
a name-based claim of architectural impossibility.
