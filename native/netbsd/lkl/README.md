# Native NetBSD LKL diagnostic build

This is an ordinary native host-process filesystem driver investigation. It
does not build a boot kernel, modify installed FUSE libraries, install a driver,
enable a Lurker filesystem, or change the qualified package. Its files belong
outside Lurker's private backend bundle.

The first phase captures unmodified current-upstream diagnostics. Run as an
ordinary NetBSD amd64 user on a work-specific filesystem with at least 6 GiB
initially free, using the native compiler and GNU make:

```sh
python3.14 scripts/build-netbsd-lkl.py --work-dir /path/to/lkl-work \
  --receipt /path/to/lkl-work/baseline.json --timeout 600
python3.14 scripts/test-build-netbsd-lkl.py
```

After a native build, set `LURKER_NETBSD_LKL_SOURCE` to its `linux-REVISION`
directory when running the tests to also compile the actual generated UAPI
layout assertions. These check stat ownership offsets, preserved ICMP padding,
packet/signal structures and directory entries, with native aliases restored.

The source is pinned to `d0f76a77e8c2ca68bd7d96190b8da09040eab072` and archive
SHA-256 `ef6e5b7cd66c023bc16193013fdacbc1285b9eb86df864b8c6cefe0ad4e0e168`.
Both cached archives and every upstream file are verified before resuming a
baseline. Existing receipts and logs are preserved. A deadline terminates the
owned make process group, including its compiler children. Build failures are
diagnostics, not qualified filesystem support.

After capturing the baseline, install the ordinary build dependencies `bison`,
`gmake`, `gsed`, and `coreutils`. The `--adapted` flag applies the hash-pinned
host patch and reuses the additive flush patch from
`native/macos/lkl-darwin-flush.patch` without changing existing disk-registration
semantics. It selects native ReFUSE API 38, GNU stat/sed aliases, BSD host mode,
and no MMU/KASAN or native networking transport. It refuses non-regular images
and requires `-f -s` if eventually qualified for execution. The patch currently
does **not** implement the credential/errno gates below: do not mount its output.

The private include overlay links to the actual `/usr/include/fuse*.h` headers.
It provides native byte-swap aliases for the compiler's `modpost` program and an
unmodified portable ELF file-format header from glibc 2.42, SHA-256
`fd6e7f3e3c74ee9d64b9f92ce3cc31b34413ca2622a167320ea053ce9a68110f`.
That header retains its LGPL 2.1-or-later notice and uses only `stdint.h`;
no glibc implementation or runtime library is used. It supplies the
cross-architecture relocation constants missing from NetBSD's native `elf.h`.
The exact native SDK/header hashes are recorded with each adapted build.
Upstream's header export uses private aliases to the interpreter running this
builder and to GNU sed/stat; their native paths and hashes are recorded. No
system executable or global alias is replaced. The overlay also maps the
Linux-style `endian.h` include to native `sys/endian.h`.
Native endian inlines are preloaded before LKL temporarily hides `__unused`
while parsing its UAPI; header discovery uses the same include-overlay flags
as host compilation. All compatibility files stay inside the build directory.
NetBSD's native `si_*` signal-field and `ipi_spec_dst` aliases are saved and
restored around that UAPI import as well. GNU sha1sum runs the upstream generated-header checks;
the final configuration and generated input hashes are retained in the receipt.
`--target boot` builds the pinned upstream RAM-only boot test executable. It
does not run the test or mount any native filesystem; run it separately with
an owned process-group deadline after verifying its native ELF provenance.

The mmap helper never passes `MAP_FIXED`. It accepts only the exact requested
address and releases only an alternate mapping returned by its own syscall.
Native tests cover collision preservation, failure cleanup, invalid alignment,
overflow and anonymous exact reuse. A shared-file fixed-base hint may be safely
rejected; these tests do not establish MMU availability.

Native ReFUSE API/flag adaptation, safe exact-address mmap hints, Linux-to-native
errno conversion, independently authenticated request credentials, explicit
block flush negotiation and native formatter compatibility remain required
gates before even file-backed runtime controls. No privileged product use is
approved by a successful compile. Preserve the installed providers and all
unrelated storage. Any usermount or device permission grant needs separate,
scoped native controls and restoration.

The existing NetBSD ReFUSE exit/unmount and stock signal handlers enqueue a
PUFFS unmount request that the running kernel processes with `MNT_FORCE`.
Closing an attached PUFFS channel also forces its mount down. Those operations
must be excluded while the mount exists. A runtime adapter must keep serving
through a normal external unmount, verify global absence of the captured
mount identity, and then close only its owned channel. The current diagnostic
patch does not implement that lifecycle boundary; its output remains unsuitable
for runtime mounting.

The stock POSIX `SIGEV_THREAD` timer path also blocks LKL boot on this host:
NetBSD's installed `sigevent(3)` documents that notification mode as unsupported.
Native timers expired without callbacks while matched signal notifications
worked. `netbsd-timer.h` supplies a separate monotonic pthread worker for the
current LKL callback signature. Its native control covers rearming, cancellation,
self-free and explicit callback quiescence. `timer_free` never joins an IRQ/self
callback; a post-CPU-shutdown drain is required before freeing LKL memory. The
standalone header/control proof alone does not qualify LKL boot or storage.
The adapted source adds an optional `timer_quiesce` host-operation hook after
CPU/thread/clock shutdown and before `free_mem()`. The NetBSD worker is the only
provider of that hook in this build; other host initializers leave it null.
Its separately pinned header is copied into the isolated LKL source tree and
verified again on cache reuse. These changes are inside the native process's
LKL library, not the NetBSD boot kernel or installed runtime providers.

The timer-integrated native RAM-only boot suite passed all 35 upstream cases,
including syscall threads and kernel shutdown. Separate native controls checked
captured credentials, set-ID clearing, Linux errno conversion and timer drain
before freeing LKL memory. Those results establish process primitives, not a
mounted filesystem or Lurker support.

Stock ReFUSE does not register `node.fsync`; the puffs dispatcher returns success
for that absent operation, and ReFUSE's `fs.sync` is a no-op. The separate
`refuse-bridge.patch` forwards file and directory fsync requests, including data
sync hints and native errors. A NULL path and NULL handle on `fsyncdir` is an
explicit private contract for a whole-filesystem barrier. An unopened node can
likewise have a NULL handle. The eventual driver must implement these cases;
the raw upstream callbacks are insufficient. ReFUSE already installs a PID
prehook for each request. The patch clears PID before querying that prehook so
a failed caller query cannot reuse an earlier PID.

Build this isolated library as the ordinary user, with a new private output:

```sh
python3.14 scripts/build-netbsd-lkl.py --work-dir /path/to/lkl-work \
  --receipt /path/to/lkl-work/refuse-build.json --timeout 180 \
  --refuse-output /path/to/lkl-work/refuse-isolated
```

This reuses the pinned source/access patch from the independent host prerequisite
builder and requires the qualified installed puffs provider. The resulting
`librefuse-lkl.so.2` has a distinct SONAME and is never installed. Its exact
source, ELF dependencies, build commands and hashes are retained. Set
`LURKER_NETBSD_REFUSE_SOURCE` to `refuse-isolated/source/lib/librefuse` to run
the native actual-source forwarding control along with the builder tests.
That control substitutes only the puffs caller seam; it does not mount. It
checks registration, file/directory/data-only sync, native error propagation,
the NULL-handle barrier, PID changes/query failure and foreign-user admission.
The driver must pin and load this exact isolated library rather than the
installed system SONAME. The diagnostic client remains unsuitable for mounting
until its remaining credential, disk-I/O and no-force lifecycle gates pass.

`netbsd-disk.h` is the file-only native block-transport component. It admits a
held caller-owned, private, single-link regular file with a sector-aligned size;
it checks byte ranges and implements zero-data-buffer FLUSH with native `fsync`.
Each disk atomically retains its first read/write/flush failure. Later successful
requests and the final host barrier do not clear that error. The native control
injects a single error at each native syscall seam and verifies that behavior,
as well as overflow, capacity change and nonprivate-source rejection.

A separate current-LKL process control used fresh images created by native
`mke2fs`, with no native FUSE mount. The clean case negotiated and forwarded
FLUSH, retained UID/GID 1300 ownership and completed ordinary LKL unmount/halt.
One-shot write and flush failures both reached LKL fsync as EIO. In the flush
case, later syncfs, unmount and halt all returned zero while the final transport
status still reported the earlier EIO. These are injected syscall-boundary
failures, not hardware-generated errors. They prove the sticky-error requirement;
they do not establish Btrfs/XFS formatting, native mounted access or driver
lifecycle support. The raw diagnostic client still does not use this component.

https://github.com/lkl/linux/tree/d0f76a77e8c2ca68bd7d96190b8da09040eab072
https://man.netbsd.org/mmap.2
https://github.com/bminor/glibc/blob/glibc-2.42/elf/elf.h
https://github.com/NetBSD/src/blob/4299530ca04c23eff6e740a3d97d29bca16c22fe/sys/fs/puffs/puffs_msgif.c
