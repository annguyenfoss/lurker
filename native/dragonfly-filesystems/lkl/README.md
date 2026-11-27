# Native DragonFly LKL host driver

This source recipe describes a separately installed native filesystem driver.
It uses DragonFly threads, local device/file I/O and host FUSE. It is external to
Lurker's private package; it supplies no VM or network filesystem transport.

The baseline is the official DPorts `filesystems/lkl` recipe, its seven patches
and checksum-pinned upstream source listed in `recipe.json`. Compile as an
unprivileged user with one worker. Preserve baseline failures separately from
DragonFly compatibility patches. The first native baseline C failure was missing
foreign ELF relocation constants in the host `modpost` tool. Patch 0001 uses the
values already defined by that exact upstream source's x86, ARM and MIPS headers.

Runtime support is unqualified. Before admission, verify native ELF dependencies,
measured device geometry, flush/error forwarding, checked normal drain, source
and FUSE identity, actual created-file ownership, caller reads/writes and foreign
user denial. The Linux emulated filesystem credentials and errno values require
an explicit native boundary audit. Host process UID or a FUSE mount-owner gate
alone does not prove those properties. Existing positive Lurker matrices and
blocked CryFS integration are separate evidence.

UFS2 is not enabled by this recipe. Its conditional Linux write implementation
and a separate native UFS2 formatter need their own assessment and controls.

The native host-glue diagnostic now compiles and passes the 35 upstream boot
checks, the strict native UAPI layout/explicit shutdown-hook probe, and the
monotonic timer lifecycle control. These tests do not mount host storage.
Patch 0002 provides nonblocking runtime timer cancellation and an optional host
callback that drains timers after CPU/clock shutdown, before guest memory is
freed. It also selects filesystem-only host features and a non-executable stack.
Patch 0003 prevents native aliases from changing imported Linux structure fields.
The generated host headers and kernel-library object must come from the same
patched tree; the old outer Makefile does not track incremental kernel changes.
Preserve/rebuild its forwarded `tools/lkl/lib/lkl.o` when those inputs change.

`tests/timer-control.c` checks cancellation, rearm, self-free and callback drain
using the actual native timer adapter. `tests/host-contract.c` links the freshly
built library and checks imported wire layouts and the shutdown-hook ordering.
Run both on native DragonFly as the ordinary user. Full native filesystem,
flush/error, ownership and permission controls remain required before admission.

Patch 0004 advertises read-only state and, for the built-in writable host
backend, FLUSH support. It accepts the two-buffer FLUSH request and forwards it
to the existing native fsync implementation. `tests/block-flush.c` wraps that
same backend for measurement without substituting a custom disk operation
(which would bypass the advertised capability). On a new native-formatted XFS
image, normal write/fsync/readback and internal unmount pass with nine successful
native flushes. A separate fault image makes only a FLUSH request use an invalid
native FD; the real backend fsync error propagates as Linux EIO from file fsync.
Fault-case unmount errors are retained and reported. These are unprivileged
internal-LKL diagnostic controls, not host FUSE or product lifecycle proof.
