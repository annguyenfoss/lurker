# macOS backend qualification patches

`cryfs-fuse-t.patch` is an experimental patch for CryFS 1.0.3 commit
`5d0cc36785d93e12250696a3ffc928042f2e8d61`. It selects the published FUSE-T
pkg-config module, supplies the removed libc++ visibility annotation for
range-v3 when needed, and adds the byte ordering already used by
`std::less<IdWrapper>` for newer libc++ transparent comparisons.
https://github.com/ericniebler/range-v3/issues/1866

## CryFS FUSE-T replay on Buran

On 2026-11-23, one experimental arm64 binary ran both transports on macOS
27.2.1 with Xcode and SDK 27.2 and FUSE-T 1.2.9. The Buran build used Lurker
`eff3af9d07ee738e928ca2873a43a575633f953c` and the pinned CryFS source above.
It applied `cryfs-fuse-t.patch` and `native/folder/cryfs/fsyncdir.patch`
together. The experimental patch already includes the compiler fixes in
`native/folder/cryfs/macos-compiler.patch`. CryFS used the FUSE 2.9 API, and
`otool -L` showed `@rpath/libfuse-t.dylib`.

| Mount selection | Observed transport | SQLite 3.53.1 | File lifecycle |
| --- | --- | --- | --- |
| Explicit `-o backend=fskit` | FUSE-T FSKit on both mounts | WAL, FULL synchronous commit, second-connection read, truncate checkpoint, exact rows and integrity check after reopen | Passed write, fsync, truncate, rename, close, and reopen |
| Default option | FUSE-T NFS on both mounts | `PRAGMA journal_mode=WAL` returned `delete` | Passed the same file checks |

The NFS run reproduced the earlier failure to enter WAL. Both runs restored
the original mount inventory and left Buran's boot identity unchanged. The test
also verified that the CryFS process exited and the owned FSKit session
descriptors disappeared.

The experimental binary SHA-256 was
`06328620bcc9289cd4bf5285489c95bdcf69fe5ab249cc66cc3a97919c331890`.
The effective source diff SHA-256 was
`2f680220e7c5df75a7b2b2a4792466b57902d2029d0c8081040aec28357f09bd`.

## Reproduction

Use a private native source and Conan cache on Buran during an exclusive
build and test window. Start from the pinned source, apply the two patches
above and build with the existing Conan 2.32.0 executable and project lock.
The replay compiled with two workers using the offline cache.

```sh
conan build . -s build_type=Release --build=never --no-remote \
  -c tools.build:jobs=2 -o '&:build_tests=False' -o '&:update_checks=False' \
  --lockfile "$cryfs_lockfile"
otool -L build/Release/src/cryfs-cli/cryfs
```

The private receipts are in
`target/goal-validation/macos-folder-fuse-t/cryfs/`. The build commands and
provenance are in `build.sh`, `build-retry.sh`, `effective.patch`,
`provenance.txt`, and `otool.txt`. The full `build.log` remains in the
private Buran directory recorded by `remote.txt`.
`probe.py`, `fskit.json`, and `default.json` record the direct runs. The
probe sends a fresh disposable password through stdin, runs CryFS in the
foreground, records the native mount row, and uses ordinary `umount` on
the exact owned mount before reopening it with the same transport.

Lurker's macOS package baseline uses macFUSE. Track further experimental
CLI and package checks against their own artifact hashes.
[Evaluation plan](../../DIRECTIVES/PLAN/MACOS_FOLDER_FUSE_T.md) ·
[Package evidence](../../docs/macos-backends.md)
