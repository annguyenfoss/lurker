# External FreeBSD LKL host driver

This recipe builds a native FreeBSD filesystem tool, separately from Lurker's
private encryption backends and packages. It embeds Linux Kernel Library
filesystem code in a FreeBSD ELF process using native threads, disk I/O and
host-installed FUSE 2. It uses no virtual machine, Linuxulator or network
filesystem. The FreeBSD `fusefs` module and FUSE userspace library remain host
prerequisites.

The source is the official `filesystems/lkl` port's Linux revision
`86dd3afb590eccc1903611bdaa8bac87757eb80d`, with all seven port patches pinned
at FreeBSD ports revision `46cf3d9696491c61d91ecd22db11c7f7c1e62c24`.
`host-io.patch` applies after those patches. The full source is GPLv2; the
builder retains `COPYING` and exact source, patch, compiler and binary
identities in its output.

https://github.com/lkl/linux/tree/86dd3afb590eccc1903611bdaa8bac87757eb80d

https://github.com/freebsd/freebsd-ports/tree/46cf3d9696491c61d91ecd22db11c7f7c1e62c24/filesystems/lkl

Build as an ordinary user on FreeBSD amd64, with the port's native GCC 14,
binutils, GNU make, GNU bc, GNU stat, GNU sed, Bison, Python and FUSE 2 headers:

```sh
python3 scripts/build-freebsd-lkl.py \
  --work-dir /absolute/user-owned/build/lkl \
  --output /absolute/user-owned/output/lkl
```

The script downloads hash-pinned sources, applies exact patch contexts and
builds with one worker. It never installs packages or elevates. Administrator
installation and native qualification are separate from building; this tool
is not selected by Lurker while its existing write guards remain enabled.

The port changes provide these host I/O contracts:

- Writable virtio block devices advertise FLUSH and accept its header/status
  request shape. Regular files use native `fsync`; GEOM character providers
  use `DIOCGFLUSH`. An unsupported or failed flush remains an error. Every
  failed block request latches a per-disk failure, so a later successful flush
  cannot turn a transient failure into a successful drain.
- FUSE callbacks are serialized with mandatory foreground `-s -f`. Each pathname request
  gets its native request UID, primary GID and bounded supplementary groups.
  Native process birth and credential/group snapshots must match. Ambiguous,
  disappeared or PID-zero pathname callers fail closed. Each callback resets
  emulated credentials and clears supplementary groups, including error exits.
  Saved root exists only inside LKL; host credentials never change. Restoration
  attempts every field independently and verifies the result. Any failure
  permanently poisons further requests and the final acknowledgement; it never
  exits an attached service. Live request authentication is also required for
  genuine held-handle I/O. Dead-origin/PID-zero read, write, fsync and metadata
  operations are unsupported; cleanup-only release remains narrow.
- Kernel-held handle release and directory release close their existing LKL
  handles with cleared emulated groups even after the original process exits.
  This cleanup path performs no pathname lookup or new I/O authorization.
- Raw Linux errors are translated at the outward FUSE boundary to native
  FreeBSD errno values; missing xattrs use `ENOATTR`. Native open flags and
  `UTIME_NOW`/`UTIME_OMIT` values are translated as well.
- Normal drain restores the original LKL namespace root, performs an internal
  read-only remount fence, normally unmounts the inner filesystem, flushes the
  host backing provider, halts LKL and closes its source descriptor. A failure
  is reported, rather than being converted to successful teardown.

The client creates a native unmounted FUSE channel and completes `fuse_new`
and LKL filesystem startup before publishing a native mount through the fixed
`/sbin/mount_fusefs -S` descriptor handoff. Its public libfuse channel wrapper
marks initialization only after the actual INIT reply is successfully sent.
The bounded monitor captures exactly one native FSID/source/target after that
initialization and helper completion. Pending signals, initialization errors
and failed sends cannot authorize a successful drain.

The driver never calls native unmount, including failure cleanup. A signal
keeps the owned service available while its mount remains attached. Before
closing its owned channel or source, it requires the captured FSID and source
to be absent from the complete native mount table and rejects a replacement at
the original target. A moved or ambiguous mount remains retained. Monitor
cancellation/join failure also retains service resources and forces an error.
Normal administrator unmount is the separate native action; force is never a
driver fallback. The executable is PIE with a non-executable stack.

The output records the host FUSE SONAME, trusted stable library entry point and
build-time provider hash. That hash is provenance, not a permanent byte pin
across compatible administrator security updates. The recipe verifies actual
native ELF requirements and loader resolution. It never copies FUSE into its
output. A consumer must revalidate the current administrator-controlled alias,
provider, ancestors and actual resolution before privileged execution.

For a client/host-only change, `--capture-kernel-from <completed-output>` can
create an audited kernel-object/header cache. Reuse it with `--kernel-cache`;
every original source input, compiler, generated header and configuration is
checked, only the named host-library/client paths may differ, and all host
objects are rebuilt. `--restage-from <completed-output>` writes a fresh output
without recompilation only after every original factory source input, compiler,
binary and license still matches; it refreshes current host-library provenance.
Existing output and source evidence are preserved on any mismatch.

`--freebsd-host-io-version` identifies protocol `freebsd-lkl-host-io-1`.
The optional `-o drain_status=/absolute/path` accepts only a pre-created,
single-link mode-0600 regular file owned by the invoking host UID. It opens
with `O_NOFOLLOW`, holds an exclusive descriptor lock, and initially requires:

```text
freebsd-lkl-host-io-1
<64 lowercase hexadecimal nonce characters>
pending
```

The same inode subsequently contains the unchanged protocol and nonce lines,
followed by one bounded status line:

```text
ready <pid> <birth-seconds> <birth-microseconds> <boot-seconds> <boot-microseconds>
```

The only final states are `drained` or `error`, with the same process and boot
identity. `drained` is written and synced after the LKL workers have halted and
the backing descriptor has closed. Partial records do not form an
acknowledgement. An existing ready/final file cannot be reused as a pending
lease. A consumer must also persist the expected source, mapping, session,
status-inode and process identities before trusting the acknowledgement;
the nonce string alone does not authorize encrypted transport teardown.

Native qualification must establish real host-flush forwarding and failure
propagation, ready/drain/error and stale-lease behavior, caller-created inode
ownership and modes, own-file updates, foreign-user denials, native errno
behavior, complete payload reopen cycles and normal closure. Functional
upstream direct-I/O results are retained separately from those gates.
