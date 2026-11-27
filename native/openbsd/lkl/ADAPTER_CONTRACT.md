# Proposed native OpenBSD filesystem adapter boundary

This is a source contract for review, following the qualified native library
and RAM-only controls. It does not authorize a FUSE mount, formatter/device
experiment, driver installation or product capability. Keep the reviewed host
patch `8d418903144152e49276e2b2b52071d31170bd7f0b29bdee926ed1bbbba63be6`
and exact static library `d07dbeda5039932129d5a66dd232fd8d92c5d4e0aafecdbb98f718fe0070d4d3`
unchanged while developing independent adapter primitives.

## First scope

Build a separate native executable against actual base FUSE API26 and its
external administrator-controlled library. Use explicit operation signatures,
not casts from API35. Initially accept only a held, verified, exclusively locked
regular-file backing descriptor on native FFS. Refuse raw devices, partitions,
vnd/softraid/VeraCrypt providers, arbitrary disk geometry and automatic partition
selection. No formatter is implied: native XFS/Btrfs creation remains an
independent source/build/format-control gate. All artifacts remain outside
Lurker's private backend bundle and installed prerequisite paths.

Compilation, synthetic callback tests and RAM-only emulated credential tests
precede any separately reviewed file-backed filesystem experiment.

## Request identity and serialized credentials

The installed headers and pinned source agree on a critical distinction:
`fusebuf.c:123` writes `p_tid + THREAD_PID_OFFSET`, while
`fuse_session.c:637` copies that value into the request context's pid field.
`sys/sysctl.h:630` exports the same encoded value as kinfo_proc.p_tid; p_pid is
separate. Match encoded THRID directly. Never query it as a PID, subtract an
assumed offset, infer identity from daemon UID or substitute directory-service
group membership.

The native snapshot primitive should use bounded, size-checked KERN_PROC
SHOW_THREADS enumeration to find exactly one live matching p_tid. Bind its
p_pid and p_ustart_sec/usec, real/effective UID/GID, complete group vector and
live thread identity. A second bounded process/thread snapshot must agree;
request uid/gid must match. Reject zero/unknown/dead/reaped/duplicate THRIDs,
changed birth or credentials, malformed/truncated counts and group vectors
larger than the exported KI_NGROUPS. Do not silently truncate supplementary
groups. Native controls must include separate pthread THRIDs in one process,
a child process, exited/reaped thread, wrong UID/GID, changed groups, and a
real secondary-group-only case.

The kernel request carries uid/gid but no full supplementary group snapshot.
The exact timing guarantee for a concurrent same-UID/GID group change needs
explicit review: two matching native snapshots demonstrate stability over that
interval, not historical group membership when the request was queued. Do not
claim that the existing wire format authenticates an unavailable historical
group vector. Any restricted initial caller policy must be explicit, tested
and acceptable to the intended product before mount qualification.

One request thread owns one emulated task and a single credential lock. Enter
verified emulated real/effective filesystem UID/GID/groups before pathname I/O;
check every transition and query-back. Restore the saved privileged task state
before releasing the lock. Any restore failure makes the session permanently
non-clean and rejects later ordinary operations while retaining service and
backing state. Test restoration failures without touching host credentials.
Release/flush callbacks from a later caller or a kernel thread require an
explicit held-handle policy; never bypass authentication by defaulting to root.
Initialization and final sync/teardown have separate daemon-only contexts.

## Native ABI and callback results

Audit every API26 operation field, Linux/native open flags, file types/devices,
stat/statvfs widths, offsets, utimens values and directory offsets. Native
mknod/open behavior must cover creation because the kernel does not use the
high-level create callback. Preserve actual emulated ownership/modes; no forced
owner reporting or permission rewriting.

Translate negative generated LKL_E* errors only at each outward FUSE callback
boundary. Keep positive byte counts/offsets unchanged, preserve the original
raw diagnostic and reject unknown negative codes as native EIO. Use guarded
aliases and contextual ENODATA-to-ENOATTR handling, not duplicate switch cases.
Native controls must exercise ENOTEMPTY, EEXIST, ENOSYS, EOPNOTSUPP, ENOATTR,
large read counts and invalid/unknown results against actual headers. Prove
native caller read/write and unrelated/group-only permissions independently.

## Ordered writes and sticky failure

Repair and test virtio FLUSH feature negotiation and the two-buffer flush
request before treating host fsync as a complete barrier. Preserve the actual
native backing error; short I/O and failed flush remain sticky even if a later
request succeeds. The regular-file callback must target the held verified FD,
not reopen a pathname. Native block-cache flushing remains outside first scope.

The native kernel returns directory-fsync success without calling the driver.
Therefore serialize every namespace/metadata mutation and require checked LKL
syncfs plus a host-FD barrier before its successful FUSE reply. Retain the first
raw filesystem/backing failure for the session, because syncfs errseq advances.
Existing atomic namespace operations remain atomic; do not publish partial
new files or weaken encryption-engine publication. File fsync/flush and final
teardown use the same error latch. Fault controls must record actual ordering,
failed writes, short writes, failed barriers, successful later I/O and a final
non-clean result. No generic power-loss durability claim follows from fsync.

## Startup and normal teardown

Do not use fuse_main/setup/teardown, stock signal handlers, or fuse_unmount.
The base session loop exits on EINTR; fuse_unmount closes the channel then may
call unmount with MNT_FORCE. Either path can destroy a still-mounted service.
Use public channel/session APIs and a separately reviewed foreground event loop
that retries interruption, remains serving while mounted, and distinguishes
normal FUSE_DESTROY from an unexpected channel failure. Handle signals through
a safe control notification; never terminate serving merely on a stop request.
Keep the channel open while the exact captured native mount fsid exists.

Record startup as pending until exact source/backing identity, native fsid and
actual request servicing are proved. Startup timeout must not kill an already
mounted service. An external normal unmount runs while the request loop stays
alive. Busy unmount retains mount, serving process, credentials state and FD.
Require exact-fsid absence before channel destruction. Then check the emulated
sync/read-only-remount/unmount chain, halt, timer/thread quiescence, final held
backing barrier and source-close acknowledgement. Any failed phase retains its
relevant live state and non-clean receipt; no forced cleanup or success exit.

At Linux6.12 setup.c:152, a negative lkl_sys_halt returns before any cleanup.
The actual native RAM control proves EPERM leaves running=true/quiesce=0 and a
subsequent restored-credential retry succeeds. The filesystem adapter must not
call lkl_cleanup, free state or close backing after a rejected halt. Positive
halt must complete the reviewed quiescence hook before free_mem.

## Review gates before storage

1. Native compile/type audit, generated errno/flag/layout checks, thread/process
   snapshot controls, RAM-only emulated credential transitions and restore-fault
   controls; no FUSE mount or disk registration.
2. Reviewed regular-FD I/O/FLUSH/error-latch controls and lifecycle state-machine
   fault tests; no formatter or filesystem mount.
3. Separate approval of the concrete file-backed native formatter and first
   mount/control plan, including normal-unmount/busy/error preservation and
   exact native fsid monitoring. Preserve every first failure.
4. Actual filesystem payload/ownership/group/foreign-denial/durability/lifecycle
   evidence before any installed prerequisite, raw provider, shared core/UI or
   matrix eligibility claim.
