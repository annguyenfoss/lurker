# Optional macOS filesystem tools

The CLI can create and mount VeraCrypt file containers with `--filesystem ext4`
or `--filesystem xfs` when the optional private tools below are staged beside the
CLI or helper. Native NTFS and ZFS create/RW routes are also available with the
prerequisites below. UFS2 remains reader-only and cannot satisfy Create + Read/Write.
Existing Apple, APFS and exFAT
defaults are unchanged. These are source-built development tools; current release
packages do not bundle them.
The native receipts are in
`DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md`.

Both routes use the existing private FUSE-T VeraCrypt engine and its unchanged
cipher/KDF profiles. VeraCrypt opens its regular decrypted `volume.dmg` through
FUSE-T SMB. Lurker detaches the unused native virtual device, which otherwise
locks that file, and runs an unmodified filesystem tool against the regular
file. The encryption service stays live throughout the session. XFS uses FUSE-T
SMB; ext4 uses FUSE-T NFS. Neither route requires enabling an FSKit module or
running a private executable as root.

Create requires at least 384 MiB. Alternate Apple-image filesystem creation
remains unavailable. There is one mount/unmount workflow without an access-mode selector.
Only AES/password PDF/text persistence was exercised for these new
routes; no desktop, physical-device, reboot, SQLite, recovery or metadata-parity
qualification is implied.

## Native NTFS, ZFS and UFS2

All three routes keep the existing private FUSE-T VeraCrypt encryption engine.
They add inner-filesystem support without changing encryption profiles.

| Filesystem | Creation | Mounted access | Additional free prerequisites |
|---|---|---|---|
| NTFS | `--filesystem ntfs`; login-user `mkntfs` formats the identified hdiutil device | Read/write through TT NTFS Native FSKit; Apple's reader alone is insufficient | Homebrew `gromgit/fuse/ntfs-3g-mac` and enabled TT NTFS Native extension |
| ZFS | `--filesystem zfs`; native `zpool` creates one pool on the identified hdiutil device | Read/write native OpenZFS | Matching OpenZFSonOsX package, approved kernel extension and administrator authorization |
| UFS2 | Unavailable | Cannot write; reader control is diagnostic evidence, not a completed Create + Read/Write workflow | Private patched `fuse-ufs` reader and separately installed, approved macFUSE kernel extension |

NTFS formatting and mounting run as the login user. ZFS elevates only trusted,
administrator-owned installed system/OpenZFS tools through `sudo -n`; it requires
existing authorization and never elevates a private executable. It records the
pool and sole-vdev GUIDs, imports only that device, and normally unmounts and
exports the pool before closing encryption. Multi-device pools and additional
datasets are outside this adapter's scope.

Stage `fuse-ufs` in the same private filesystem directory described below.
Its pinned source and the two small Darwin compatibility patches are documented
in `native/macos/fuse-ufs.md`. macFUSE is an additional optional filesystem
runtime; VeraCrypt continues to use FUSE-T. On Apple silicon, enabling these
kernel extensions requires the operator's startup-security and extension
approval steps. The tools remain free of charge.

## Reproduce the native tool builds

Build as the login user on macOS with FUSE-T 1.2.9, Xcode command-line tools,
Homebrew e2fsprogs, autoconf, automake, libtool and pkgconf already available.
Use an isolated working directory. Pin the following unmodified source commits:

- fuse-xfs: `f940a1d3e8cbb7a54897eefb34e522042c61e393`
  https://github.com/karan-vk/fuse-xfs
- fuse-ext2: `ae35afb9ab08d87c66c1e021df792b3a7c4308b0`
  https://github.com/alperakcan/fuse-ext2

For fuse-xfs, run from its `src` directory:

```sh
make -j4 \
  FUSE_CFLAGS='-D_FILE_OFFSET_BITS=64 -DFUSE_USE_VERSION=26' \
  FUSE_INCLUDES='-I/usr/local/include/fuse' \
  FUSE_LDFLAGS='-L/usr/local/lib -Wl,-rpath,/usr/local/lib -lfuse-t'
```

For fuse-ext2, run from its source root:

```sh
autoreconf -fi
./configure --prefix="$PWD/private-stage" \
  CPPFLAGS='-I/usr/local/include/fuse -I/opt/homebrew/opt/e2fsprogs/include -D_FILE_OFFSET_BITS=64' \
  LDFLAGS='-L/usr/local/lib -Wl,-rpath,/usr/local/lib -L/opt/homebrew/opt/e2fsprogs/lib' \
  LIBS='-lfuse-t'
make -C fuse-ext2 fuse-ext2 \
  CFLAGS='-g -O2 -Wno-error=incompatible-function-pointer-types'
```

The compiler compatibility flag permits the old upstream Darwin `getxattr`
callback signature. No source patch was applied. The independent installer GUI
target fails on current SDK headers and is not needed for this driver target.
Extended attributes remain unqualified; these experimental upstream drivers
must not be treated as complete filesystem implementations.

Stage `fuse-xfs/build/bin/fuse-xfs`, `fuse-xfs/build/bin/mkfs.xfs`,
`fuse-ext2/fuse-ext2/fuse-ext2` and the native e2fsprogs `mkfs.ext4` under one of:

- `<CLI-or-helper-directory>/macos-filesystems/`
- `<CLI-bin-directory>/../libexec/lurker/filesystems/`
- `<App.app>/Contents/Helpers/filesystems/`

Preserve their required native libraries and license notices when distributing
any artifact. The qualified development stage depends on the separately
installed FUSE-T and Homebrew e2fsprogs libraries; this is not a relocatable or
offline-qualified release package.

## Lifecycle and retained failures

A private per-user registry records the encrypted source inode, decrypted image
inode and auxiliary mount, driver PID/start time, and mounted filesystem identity.
Drivers start in their own session so closing a CLI terminal does not send them
SIGHUP. Close first asks `diskutil` to unmount normally, stops only the identified
filesystem driver after the filesystem is drained, and then closes VeraCrypt.
Changed identities or failed closes preserve the encrypted session for recovery.

Direct Apple-image device experiments are retained as failures. NTFS-3G rejects
unprivileged block devices; Darwin raw devices cannot satisfy its unaligned
reads, and VeraCrypt's SMB file transport does not provide NTFS-3G's required
record locking. The qualified TT NTFS route avoids that failed mounting path:
`mkntfs` formats the native virtual device and FSKit supplies read/write access.
