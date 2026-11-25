# Optional macOS ext4 and XFS container tools

The CLI can create and mount VeraCrypt file containers with `--filesystem ext4`
or `--filesystem xfs` when the optional private tools below are staged beside the
CLI or helper. Existing Apple, APFS and exFAT defaults are unchanged. These are
source-built development tools; current release packages do not bundle them.
The native receipts are in
`DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md`.

Both routes use the existing private FUSE-T VeraCrypt engine and its unchanged
cipher/KDF profiles. VeraCrypt opens its regular decrypted `volume.dmg` through
FUSE-T SMB. Lurker detaches the unused native virtual device, which otherwise
locks that file, and runs an unmodified filesystem tool against the regular
file. The encryption service stays live throughout the session. XFS uses FUSE-T
SMB; ext4 uses FUSE-T NFS. Neither route requires enabling an FSKit module or
running a private executable as root.

Create requires at least 384 MiB. Explicit Apple-image ext4/XFS creation and NTFS
writable creation remain unavailable. NTFS read-only imports keep the native
route. Only AES/password PDF/text persistence was exercised for these new
routes; no desktop, physical-device, reboot, SQLite, recovery or metadata-parity
qualification is implied.

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
record locking. None of those failures is advertised as writable support.
