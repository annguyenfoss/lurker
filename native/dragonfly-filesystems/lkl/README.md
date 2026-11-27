# Native DragonFly LKL host driver construction

This is a separately installed native host filesystem prerequisite. It uses
DragonFly threads, local FD I/O and the stock host FUSE facility, with no VM,
network transport or syscall-hijack library. It must never enter Lurker's app or
private crypto bundle. The source, official DPorts patches, ordered DragonFly
patches and required header hashes are pinned in `recipe.json`.

Apply every patch and source header before a fresh one-worker build. The old
outer Makefile does not track incremental kernel changes: a changed kernel or
generated host ABI requires rebuilding its forwarded `tools/lkl/lib/lkl.o` and
headers. Incremental diagnostic receipts are not a fresh factory qualification.
UFS2 is not enabled; the native client accepts only Btrfs and XFS. Native
formatter construction is separate from imported-image driver controls.

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
so mapped/raw devices remain a separately reviewed lease gate. Captured native
FSID/source/target must be globally absent before drain and channel/session
disposal. Signals keep service alive and latch an unsuccessful final result; moved/replaced/uncertain
mounts retain the source/channel. Ordinary user-chosen mountpath admission needs
a separately reviewed supervisor/FD-bound target contract.

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
client remains unqualified pending root review/trusted copy, actual native INIT,
caller/foreign permissions, ownership/mode/time/set-ID checks, opener-death and
inherited-descriptor controls, full payload/reopen and physical geometry tests.
No Lurker product or matrix pass follows from these construction receipts.

The pinned kernel returns a negative halt before CPU/thread/timer cleanup. Both
startup-error and ordinary shutdown therefore retain the channel and backing FD
while retrying at50ms intervals. Only an actual halt0 permits disposal; a prior
negative result remains a final error. `tests/halt.c` proves a real reboot EPERM
leaves the kernel running, native channel attached and source flock held, then
verified root restoration permits halt0. The separate test-only halt driver can
pause the retry for a controlled mounted retention test; that privileged test
remains pending.

`fuse_parse_cmdline` adds a kernel-only default fsname. The native controller
supplies the validated FUSE-device alias independently, so the client removes
only fsname before calling the high-level unmounted constructor. Every other
option still reaches its normal parser. `tests/constructor-options.c` verifies
the actual argument pipeline and native constructor, plus rejection of an
unrelated unknown option. The first reviewed024 attempt failed on the synthetic
fsname before readiness/publication; its untouched fixture and failure receipt
are retained.
