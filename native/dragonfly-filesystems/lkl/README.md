# Archived DragonFly LKL host driver experiment

**Cut on 2026-11-27.** This prototype injects a new embedded-kernel ABI slot and
shutdown hook, which falls outside Lurker's non-invasive kernel integration
boundary. Its kernel-dependent builds, controls, mounts and product admission
are removed from actionable plans. Do not build, run or resume this recipe.
No kernel workaround or upstream-interface project is planned. See
[SCOPE_CUT.md](SCOPE_CUT.md) for completed work, provenance and the verified cut
state. Source, partial artifacts, failed fixtures and evidence are preserved.

The remainder describes historical diagnostic code, not installation or
validation instructions. It used native DragonFly threads, local FD I/O and
stock host FUSE, without a VM, network transport or syscall-hijack library.
The pinned source and patch manifest remains in `recipe.json` solely for
reproducibility of the record. It must never enter Lurker's app or private
crypto bundle. No Btrfs, XFS or UFS2 product pass was established by this route;
UFS2 was never enabled. Independent native formatter and userspace FUSE work
remains separate from the cut embedded-kernel experiment.

The host adapter supplies native thread identity and monotonic one-shot timers.
Its explicit shutdown hook drains timer callbacks after CPU/clock shutdown and
before guest memory is freed. Native UAPI layout/timer controls and all35 upstream
RAM boot tests passed; these alone provide no host-storage runtime credit.

Regular backing files require positive512-byte-unit sizes. Character devices
require native DIOCGPART512-byte sectors, consistent media blocks/bytes and an
agreeing SEEK_END result. Uncertain geometry fails before block publication.
The built-in writable backend advertises FLUSH and forwards it to native fsync.
Any failed block request latches a per-disk I/O error until removal. This matters:
a genuine native FLUSH failure shut down XFS while remount/unmount still returned
zero. The final native internal control now reports drain=-EIO for that failure;
normal fsync/readback/unmount reports zero. Original-root FD restoration, checked
read-only remount/unmount/native fsync and the sticky status govern drain.

Each FUSE return boundary translates symbolic generated LKL errno values to
native errno values. Internal comparisons stay Linux values; positive results
remain unchanged. Only xattr ENODATA becomes ENOATTR. Unknown errors are logged
and fail as EIO.

Pathname and metadata operations require two complete, stable native process
snapshots agreeing on PID, birth, live state, effective UID/GID and the complete
group vector. The context umask is explicit; inherited groups are cleared before
applying a request. Every callback verifies restored real/effective/saved UID/GID,
FSUID/FSGID zero, no supplementary groups and umask zero. Failed restoration
poisons further filesystem requests until complete restoration is verified; it
never exits a published process. The failure stays sticky after recovery and
forces final exit1. Registered close/release and the upstream no-op flush remain
cleanup-only capabilities, with no pathname or ambient-root authorization.
Invalid, mismatched, dead and PID-zero pathname identities
fail closed. No caller umask is inferred from kinfo_proc.

DragonFly sends actual cached WRITE requests with PID zero. Registered opaque
handle tokens therefore retain the validated opener identity/groups/umask and
exact held Linux FD/directory. PID-zero data I/O uses that handle's opener,
without namespace-root privilege or pathname authorization. Close/release accepts
only registered handles; current authentication remains required for metadata
changes. The original native baseline's UID1300 writer created UID0/GID0 children;
that failure is retained and supplies no caller-support credit.

The client uses the native exported unmounted FUSE2 channel constructor. It never
calls fuse_mount, fuse_unmount or their force-capable BSD helpers. It requires a
foreground single-threaded controller, protected administrator-owned mountpoint
and alias ancestry, an inherited exact /dev/fuse vnode and a readiness pipe. LKL
and the FUSE session are prepared before the controller publishes the mount.
The backing FD takes LOCK_EX|LOCK_NB before block-device registration and
retains that claim until final drain/close. Same-file and hardlink claims conflict;
symlinks are refused by O_NOFOLLOW. Native character-node flock is unsupported,
so mapped/raw device admission was never qualified and is cut with this route.
Captured native FSID/source/target must be globally absent before drain and
channel/session
disposal. Signals keep service alive and latch an unsuccessful final result; moved/replaced/uncertain
mounts retain the source/channel. Ordinary user-chosen mountpath admission was
never qualified; its dependent supervisor work is cut with this route.

The scoped FUSE2 static library recipe is in `../fuse2`; the global installed
provider is unchanged. Its64-byte native INIT reply preserves the negotiated
7.19 prefix/features and zeroes40 extension bytes. The observed channel checks
the actual INIT bytes/send result and latches transport failures. Native public
channel delegates temporarily bind the sole session to the native inner channel
and restore the outer channel on every path. Unexpected termination is never
normalized to success. Explicit DESTROY and checked global absence govern close.

Native unprivileged controls cover UAPI/timers/boot, symbolic errno, real fsync
failure forwarding, driver internal drain, capacity validation, complete
credential apply/restore and native channel attachment/error restoration.
Standalone actual callbacks additionally prove PID-zero held I/O, set-ID clearing,
genuine restoration EPERM, unchanged data while poisoned, registered cleanup and
verified recovery with historical failure retained. Fault activation exists only
in separately compiled test translation units, never a production option. A
native ELF with the scoped PIC FUSE2 archive links successfully. The new mounted
client was not qualified: actual native INIT, caller/foreign permissions,
ownership/mode/time/set-ID checks, opener-death and inherited descriptors, full
payload/reopen and physical geometry controls did not complete. These dependent
controls are cut, not an actionable validation queue.
No Lurker product or matrix pass follows from these construction receipts.

The pinned kernel returns a negative halt before CPU/thread/timer cleanup. Both
startup-error and ordinary shutdown therefore retain the channel and backing FD
while retrying at50ms intervals. Only an actual halt0 permits disposal; a prior
negative result remains a final error. `tests/halt.c` proves a real reboot EPERM
leaves the kernel running, native channel attached and source flock held, then
verified root restoration permits halt0. The separate test-only halt driver can
pause the retry for a controlled mounted retention test; that privileged test
was not executed and is cut with the dependent route.

`fuse_parse_cmdline` adds a kernel-only default fsname. The native controller
supplies the validated FUSE-device alias independently, so the client removes
only fsname before calling the high-level unmounted constructor. Every other
option still reaches its normal parser. `tests/constructor-options.c` verifies
the actual argument pipeline and native constructor, plus rejection of an
unrelated unknown option. The first reviewed024 attempt failed on the synthetic
fsname before readiness/publication; its untouched fixture and failure receipt
are retained.

The native inventory runs only in a separate host observer, never in served
callbacks. Stock DragonFly's NOWAIT mount scan skips unmounting entries, so an
empty inventory alone cannot authorize cleanup. The observer first binds the
exact FUSE/source/target FSID and requires its native `fhstatfs` to return the
stock FUSE default `EOPNOTSUPP`. Later cleanup requires both empty inventory and
that same typed FSID returning `ESTALE`. This relies on the reviewed stock
mount-tree lookup and FUSE's default file-handle stub; it is not a generic
synthetic-file-handle absence test. Query uncertainty retains the channel/source,
and observer join must succeed before disposal.

Protected diagnostic controllers must launch outside chroot and jail. The client
checks root credentials, `jail.jailed=0`, and an unchanged held root-directory
identity; those checks alone do not prove an arbitrary launch is outside chroot.
A general product supervisor was not qualified with this route. Query-result and
thread controls are in `tests/observer.c`; `tests/fsid-probe.c` is read-only.
The separate test-only observer driver holds the actual DESTROY callback before
its reply, allowing present/unmounting/absent native syscall observations without
closing the source early. Actual mounted controls were not executed and are
cut with this dependent route.
