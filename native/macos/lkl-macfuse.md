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

## Remaining mounted macFUSE adapter plan

No replacement driver is installed by this recipe yet. Buran's xlinuxfs FSKit
extension still lacks the restricted provisioning profile. Do not bypass that
entitlement. The separate macFUSE route can avoid that transport requirement.

The installed root-owned `/usr/local/lib/libfuse3.4.dylib` contains an arm64 slice
and links macFUSE's MFMount framework; its FUSE 3 headers accept API 35. This is
distinct from FUSE-T's static library. The companion `tools/lkl/lklfuse.c` uses
FUSE 3 API 35; a native compile probe found Darwin stat timestamp names and
Linux-only open flags need adaptation. It is not a drop-in Mac driver. Using
FUSE 2 would require a separate callback ABI port and is not established here.

Before installation, review the adapter diff against these gates:

1. Use explicit native macFUSE 3 headers/linkage and the pinned Darwin archive.
   Map Darwin timestamp structures and supported open flags deliberately.
   Convert every Linux errno through native constants, including ENOTEMPTY,
   ENOSYS, EOPNOTSUPP/ENOTSUP and xattr errors. EIO's shared numeric value is not
   evidence for those mappings. Probe missing/duplicate/unsupported/nonempty,
   busy and denied operations independently.
2. Propagate each request's `fuse_get_context` credentials and supplementary
   groups into a serialized LKL request context, restoring them on every exit.
   Never serve requests with emulated root credentials by default or change host
   process credentials to obtain backing access. Test new-child uid/gid/mode,
   updates, setgid inheritance and foreign denial through searchable parents.
   Mount-owner admission or synthesized stat ownership cannot replace this.
3. Open file backings as the login user where native permissions allow. Keep
   administrator-executed driver code, ancestors and dependencies root-owned and
   unwritable by users. No elevation of a private encryption engine or a
   user-writable driver; no generic privileged arbitrary-device shortcut.
4. Expose per-file and directory fsync with propagated errors. Do not reuse
   `lkfs_sync`'s unconditional success as a durability result. Drain and unmount
   LKL while the backing remains available, and retain it on busy/error.
   Test exclusive backing claims, symlinks, duplicate mounts, busy close, normal
   worker exit and backing reopen before considering product integration.
5. Prove a host-visible local macFUSE mount with actual driver PID, binary/hash,
   source/backing identity, mode and no network/guest transport. Run common
   payload and refusal controls there before replacing the product XFS guard.

The proposed later owned changes are `native/macos/lklfuse-darwin.patch` and
`scripts/build-macos-lkl-filesystem-driver.py`. They build/install a separate
host prerequisite, never add filesystem code to the Lurker bundle.

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
