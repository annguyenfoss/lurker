# NetBSD native FUSE access prerequisite

These are host libraries, installed separately from Lurker. Never place them in
Lurker's private backend directory or include them in its packages. The pinned
NetBSD 11 source and correction were natively qualified on NetBSD 11.1 amd64.

The stock userland stack has three demonstrated access problems: native vnode
access bits differ from the stable libpuffs API bits, libperfuse can use stale
cached attributes, and ReFUSE lacks complete access and mounting-user admission.
Reported inode ownership/modes alone do not prove permission enforcement. Native
controls reproduced foreign-user reads despite private modes.

`fuse-access.patch` corrects the userland access boundary without changing the
kernel, cache flags, flush behavior or encryption profiles. It accepts native and
legacy basic access bits, keeps libperfuse attributes current, and applies FUSE
mounting-user admission across credential-bearing ReFUSE operations. Standard
`allow_other`, `allow_root` and opt-in `default_permissions` are handled explicitly.
The original upstream notices remain in the source and build output.

Upstream source:
https://github.com/NetBSD/src/tree/4299530ca04c23eff6e740a3d97d29bca16c22fe/lib

Build as the ordinary user on the target NetBSD host, using a new private work
directory. The script pins all 90 source files, verifies the installed public
puffs ABI header, applies the exact reviewed patch, builds with the native C
compiler and verifies the capability functions in a separate native process.

```sh
mkdir -m 700 /path/to/netbsd-fuse-work
python3 scripts/build-netbsd-fuse-prerequisite.py \
  --cache /path/to/netbsd-fuse-work/cache \
  --output /path/to/netbsd-fuse-work/output
```

The output contains `libpuffs.so.2`, `libperfuse.so.0`, `librefuse.so.2`, a manifest,
the build log, the applied patch and the exact source tree with its notices.
The builder does not modify the host installation. Do not run a user-writable
builder or library as an elevated Lurker backend.

An administrator installs the libraries as a host prerequisite:

1. Close every FUSE/PUFFS mount normally and verify its daemons have exited.
2. Preserve the current native library files, symlinks, permissions and SHA-256
   hashes in a root-private backup. Resolve the actual versioned targets behind
   `/usr/lib/libpuffs.so.2`, `/usr/lib/libperfuse.so.0` and
   `/usr/lib/librefuse.so.2`; do not guess a different installed version.
3. Copy the reviewed output into an administrator-owned staging directory and
   verify its manifest and library hashes there. Install each matching library
   atomically at its native versioned target, preserving root ownership and the
   original read-only library permissions. Do not change unrelated libraries.
4. In a fresh process verify each native capability function returns `1`:
   `puffs_access_abi_version`, `perfuse_access_cache_version`, and
   `refuse_access_policy_version`. Verify actual loaded native paths, rather than
   only executable arguments or predicted dependency paths.
5. Recheck caller access and unrelated-user denial using a traversable test
   parent: the caller must list/read/write, sync, append, close and reopen; an
   unrelated UID must be denied listing, reading and writing. A nonzero command
   exit after printing private data is not a privacy pass.

Restore the backed-up native libraries if those checks fail. Later OS updates
may replace these libraries, so Lurker checks the native capability versions
before starting affected filesystem operations.

Ordinary-user CryFS additionally needs administrator-enabled
`vfs.generic.usermount=1` and read/write group access to the native puffs device.
Use a narrowly scoped group rather than world-writable device permissions.
Lurker does not silently change either prerequisite. These access corrections do
not add shared-memory mapping to ReFUSE; the separately recorded SQLite WAL
limitation remains.
