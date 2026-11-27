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
  The pinned halt function returns a negative error before worker shutdown and
  joining. Every client halt path checks that result: a failure retains the
  source, owned channel and status lock, reports an error, and retries at bounded
  intervals. Only halt success permits source closure, and the failure stays
  sticky even when a later retry succeeds.

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
The qualified published-SIGTERM control retains ready status and service, then
exits zero with drained after the later normal administrator unmount; the
signal itself is not a sticky I/O or shutdown failure.
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


## Kernel-domain provenance and current scope hold

The current goal permits only already published/tested mainstream kernel
patches, including embedded LKL. In-process execution does not exempt this
source from that restriction. New LKL builds, kernel-domain changes, admission
and storage tests are held pending dependency classification. Existing artifacts
and failed fixtures are preserved.

The seven pinned FreeBSD port patches are published distribution inputs:

| Target | Published port change |
| --- | --- |
| `arch/lkl/Kconfig` | PowerPC 64-bit/big-endian selection; inactive for amd64 |
| `arch/lkl/Makefile` | Extend PIC format list to PowerPC |
| `arch/lkl/include/uapi/asm/host_ops.h` | Increase saved jump-buffer storage from 32 to 128 words |
| `tools/lkl/Makefile` | Clear outer LDFLAGS for nested kernel build/install |
| `tools/lkl/Makefile.autoconf` | Extend native POSIX/FreeBSD host selection to PowerPC |
| `tools/lkl/Targets` | Omit the archive-conversion utility targets |
| `tools/lkl/lib/virtio_blk.c` | Correct 64-bit sector/capacity endian conversions |

Official source/package publication is established; it does not by itself
establish popularity or an independent runtime test for every patch. These are
separate from the local `host-io.patch`, which modifies five targets:

- `arch/lkl/scripts/headers_install.py`: a local kernel/header-build script
  change forcing multiprocessing `fork` for the native Python 3.14 build.
  The builder also performs the port's ordinary interpreter-shebang substitution.
- `tools/lkl/lib/virtio_blk.c`: local virtual block-device FLUSH advertisement,
  header/status-only request handling and a sticky I/O-error latch. These are
  kernel-boundary contract changes even though compiled in the host library;
  current durability/error controls depend on them.
- `tools/lkl/lib/posix-host.c`: local native provider capacity and host fsync/
  GEOM DIOCGFLUSH forwarding. Current raw-provider controls depend on this glue.
- `tools/lkl/include/lkl_host.h`: local host-library flush/error-query declarations.
- `tools/lkl/lklfuse.c`: native userspace FUSE/errno/credential/direct-I/O and
  checked lifecycle/halt/status integration.

No local modifications were made to `fs/btrfs`, `fs/xfs`, `kernel/` or `mm/`
C sources. The kernel uses pinned upstream `defconfig` through the existing
`KCONFIG?=defconfig`; its captured configuration hash is
`2251866a011d7ad4ed9924e2fba26d0bc8f090766e24e14742faccff555fea0f`.
It enables Btrfs and XFS and leaves UFS disabled. No tailored `.config` was
written. This distinction is evidence, not permission to bypass the scope hold.

A read-only comparison of all 63,047 pristine archive entries against the
frozen final12 input manifest found exactly the eleven content changes above
(the official/local sets overlap at virtio_blk), one equivalent PowerPC
selftest symlink normalization, and extraction permission normalization
0664→0644 / 0775→0755. No other source content changes or missing inputs were
found. The detailed audit is in the round's FreeBSD evidence directory under
`validation/kernel-domain-audit/`. Local changes are not being presented as
already published/tested mainstream patches. Independent userspace guardian,
codec and Cryptomator ABI/FFM work does not link or run LKL.


The independent `guardian-platform.h` currently supplies native PID/birth/boot
and held-source identity helpers, regular-file/GEOM barrier selection and
checked close semantics. It includes no LKL code, mount operation or completion
authorization and is not wired into the held LKL build. Its strict native C11
control passes 29 checks for process identity, missing/reaped processes,
regular-file identity/resize/replacement rejection, flush and close behavior.
The character-provider branch is compiled but is not credited as a runtime
provider/guardian acceptance result. Native stream peer-credential probes are
separate evidence; a live guardian/state-machine integration remains pending.


### Guardian worker inheritance primitive

`guardian-platform.h` now supplies a pre-thread `posix_spawn` helper with the
production contract: connected progress channel FD3 and held source FD4 only.
The guardian keeps its status descriptor CLOEXEC. Input descriptors are first
duplicated above the fixed range; child actions install null stdin, private
regular-file stdout/stderr, channel3/source4, then close all descriptors from 5.
The worker immediately marks 3/4 CLOEXEC before any potential helper execution.
Executable/environment/binding trust remains the caller's responsibility;
root execution additionally rejects writable/non-root executable ancestry.
This helper inherits native credentials; a root-to-login-worker credential
transition and driver/service integration are not implemented or qualified here.

A native multi-threaded caller is refused. If duplicate cleanup fails after
spawn, an error return preserves the positive owned child PID: callers must
retain/reap it and source/session custody. This never means that no child ran.
The native user-only controls pass 36 parent checks plus the dummy-worker FD,
stdio, group and mask assertions; the prior 29 identity/source checks still pass.
Their immutable source/log receipt is `build/guardian-platform-spawn-v2/` in
the round evidence. No LKL, FUSE, mount or device operation ran, and the helper
is not wired into the installed protocol1 tool or any crypto teardown authority.

The preceding v1 gate passed 35 parent checks. V2 publishes the output PID only
after actual spawn success and adds a real executable-not-found control; an
ENOENT result leaves no published child, while a later duplicate-close error
retains the positive owned child. Both source/log sets remain preserved.

V3 aligns production spawn results with the Mac binding: `0` started, `1`
retained owned child after setup failure, `-1` no published child. Retained
results carry a positive PID and errno; the caller must latch the failure and
retain custody, not proceed as ready. Executable dev/inode/size/timestamps/mode/
owner are compared across spawn. Native v3 passes 41 parent checks plus dummy
worker assertions, including a reported metadata change and actual-close then
reported EIO. Earlier v1/v2 scopes remain historical; only v3 uses this return
contract. No root credential transition or driver integration is enabled.
