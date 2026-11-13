# CryFS stable durability patch

`fsyncdir.patch` applies to stable CryFS 1.0.3, commit
`5d0cc36785d93e12250696a3ffc928042f2e8d61`. It changes persistence only;
AES-256-GCM, 16384-byte blocks, scrypt, ciphertext format, block names and
integrity/version checks remain upstream's. Linux and macOS builds share this
patch; native macOS qualification remains outstanding.

Upstream `Fuse::fsyncdir` returned success without doing any work. In addition,
file/blob flushes could leave ciphertext in `CachingBlockStore2`, and on-disk
stores used buffered stream writes without OS fsync. A successful fsync followed
by an owned daemon crash could therefore lose committed directory entries.

The patch routes directory fsync through `FilesystemImpl` and `CryDir`, flushing
the directory blob and its parent metadata through the existing blob flush graph.
New child blobs, including empty files/directories and symlinks, are flushed
before their directory entries are published. It bypasses the production
write-back **block** cache, retaining the blob caches and parallel-access layers.
Errors from file/directory fsync reach FUSE callers; there is no global `sync()`.

Encrypted block updates write a same-directory temporary, check short writes,
fsync it, publish by atomic rename, and fsync the prefix and root directories.
New blocks use an atomic no-clobber hard link. A failed write leaves the old
committed ciphertext intact, and ordinary failures remove the temporary. Abrupt
process death can leave an ignored encrypted temporary; no plaintext is stored
there and no automatic repair or deletion is attempted.

This correctness-first implementation performs more synchronous I/O than
upstream and may slow write-heavy workloads. A smoke comparison on the guest's
/tmp tmpfs measured 32 SQLite FULL/WAL commits at 0.0388 seconds upstream versus
0.1827 seconds patched, and a 1 MiB write/fsync at 0.0012 versus 0.0041 seconds.
These tiny samples illustrate the I/O cost; they are not disk benchmarks.
It does not make a multi-block
filesystem transaction atomic across arbitrary power loss. Upstream still has
write paths in destructors: the primitive ENOSPC regression below does not qualify
the full mounted filesystem's ENOSPC recovery behavior. An additional application
probe encountered the guest's per-user /tmp EDQUOT limit and the daemon exited;
its failed vault was retained and ordinarily detached after reclaiming only
redundant test executables. The unchanged backend passed the same 32-commit
application workload once quota headroom was restored. Do not advertise complete
power-loss or full-storage recovery qualification from these results.

## Native verification

All runtime encryption/storage tests ran as the ordinary `adjutant` user on
`archlinux-core-01`, kernel `7.2.15-arch1-1`, on private `/tmp` fixtures. Endeavour
only compiled code. The final patch SHA-256 is
`49923e975f57f07fd2fe78abf649464ca5e20b34c9edf0a1cab98080bb2af0b0`;
the tested Linux backend SHA-256 is
`3c3ae33ab2c1843e1cf06812ba14b53ad54c19a64c3b4678c93f3b55b6831c97`.

- The strong `scripts/test-folder-crash.py` test passed all 10 checks on that
  exact backend: atomic file rename, file and directory fsync, SQLite WAL/FULL
  with an uncheckpointed 24752-byte WAL and an open application connection,
  verified owned-daemon SIGKILL, ordinary unmount/reopen, SQLite integrity check
  and the exact committed row/file payload. Duration: 14.28 seconds. No repair.
- `durability-test.cpp` passed no-clobber creation, replacement, real kernel
  short-write `EFBIG`, preservation of the previous ciphertext and temporary
  cleanup. The same test passed actual `ENOSPC` on an owned private 2 MiB tmpfs;
  that filesystem was ordinarily unmounted and removed after success. An
  ordinary host filesystem was never filled.
- The patch passed `git apply --check` against the pristine source pin, and the
  complete patched backend compiled successfully. These results are specific to
  the tested native Linux build; they do not qualify macOS FUSE.

To compile the standalone regression after a patched Conan Release build:

```sh
python native/folder/cryfs/build-durability-test.py \
  target/PATCHED_CRYFS_SOURCE target/cryfs-durability-test
```

Transfer the executable to the dedicated native test host, create an empty
private directory whose path starts with `/tmp/lurker-cryfs-durable-`, and pass
that directory as its only argument. `--finite-tmpfs` additionally requires that
same directory to be an already mounted tmpfs of at most 16 MiB; it refuses to
fill any other filesystem. The diagnostic does not mount filesystems, elevate,
install dependencies or run automatically during a local build.
