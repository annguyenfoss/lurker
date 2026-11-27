# Fresh native qualification round — 2026-11-27

## Authority and scope

This round implements the user's goal-objective.md supplied on 2026-11-27.
Start from new fixtures and observations on all eleven assigned hosts. Prior
reports, installed binaries and caches are discovery aids, never new pass evidence.
Preserve old work and unrelated live storage. Windows is excluded this round.

The ten filesystems are ext4, Btrfs, XFS, APFS, UFS1/FFSv1, UFS2/FFSv2,
HAMMER2, NTFS, exFAT and ZFS/OpenZFS. Assess every filesystem on each of
Arch, Debian, Ubuntu, Fedora, openSUSE, Gentoo, FreeBSD, OpenBSD, NetBSD,
DragonFly BSD and macOS. FAT32 may be a control; it does not replace any cell.

For each pair assess LUKS1, LUKS2, Apple Encrypted Disk Image (hdiutil),
APFS Encrypted (diskutil), GELI, PEFS, softraid CRYPTO, softraid RAID1C,
CGD, VeraCrypt, gocryptfs, stable CryFS and Cryptomator. Record an explicit
applicability reason for every excluded combination. Fixed-filesystem APFS
encryption is APFS-only. Directory encryption is tested on each eligible native
backing filesystem; it has no selectable inner filesystem. Do not mistake these
two meanings of filesystem. securefs and single-file crypto are outside this round.

## Eligible routes and product model

Use OS facilities or well-maintained (or demonstrably mature/stable) open-source
or permanently free software. Paid open source is eligible when its available
source can be built; a paid store binary need not be used. Reject paid-only
software and time-limited payment trials. Record license, upstream/source,
version, maintenance evidence and installation/build provenance for candidates.

All filesystem/crypto work executes on the target OS using host-local facilities.
No guest/microVM, WSL, network filesystem or network-export workaround may
provide a missing capability. The ten KVM guests are the native test hosts,
not compatibility layers nested inside a test. macOS FUSE-T is preferred where
applicable. Its existing host-local transport must be identified explicitly;
do not introduce a network filesystem to substitute for a filesystem driver.

A separately installed native driver may incorporate filesystem code from
another OS when it executes as a native host process using local host I/O and
FUSE, without a VM or network transport. FreeBSD's official `lklfuse` port fits
that eligibility boundary. Its native `direct_io` controls establish feasibility;
actual Lurker container, device and directory-backing flows still require
qualification. Keep this filesystem driver external to the Lurker package.

Lurker wraps system encryption and ordinary OS format/mount/unmount facilities.
Bundle eligible independent encryption engines privately, subject to the existing
trusted-elevation rules. Install tightly coupled drivers and FUSE on the host;
do not bundle filesystem tools/drivers or FUSE into Lurker. Expose decrypted
storage as an ordinary native disk where feasible. Detect real OS capabilities
and reuse them rather than implementing a separate filesystem stack. Custom
integration is permitted when needed. CLI and Slint desktop remain first-class
frontends over core; desktop uses the helper, never the CLI.

## Authorization, host changes and ownership

The user authorizes updates of OS/kernel, drivers and packages, installation of
missing eligible tools, necessary host configuration/extensions and beta builds
when justified. One agent exclusively owns each host and its assigned domain.
Use SSH inventory keys; Buran is adjutant@buran (fallback .local/.swarm).
Use passwordless sudo on Buran and the user-provided VM sudo credential via
stdin only, never tracked files, argv, transcripts or reported output.

Orderly VM reboot is permitted after observing job/mount state and retaining
evidence. Never reboot/shutdown Buran. Record a macOS update requiring reboot
as pending; do not claim it installed/running. The user's host authorization
covers scoped updates without another permission round. Custom kernel builds
on the ten VMs are deferred until ordinary candidates and tests have finished
and the complete ordinary-route assessment has been reported. Do not begin
them merely because one cell lacks a stock driver. Ordinary matching external
driver modules are assessed separately from a customized kernel.

For device tests attach a new small qcow2 per VM through qemu:///system libvirt.
Record domain, image, serial and guest identity before any destructive command.
Use only that verified scratch disk, never a guessed /dev name or an existing
disk. Buran's authorized Toshiba 128 GB USB requires current model/size/transport
identity and partition inventory. Keep physical Mac sampling small to limit USB
bandwidth. All applicable container cases precede device cases.

## Required work and acceptance

1. Inventory current OS/kernel, boot ID, tools/drivers, mounts/devices, jobs,
   free space and exact update outcomes. Refresh upstream eligibility evidence.
2. Update ordinary supported packages/kernel and install eligible missing tools.
   Build a fresh exact-commit artifact in isolation using managed toolchains and
   existing verified caches; record input and binary hashes. Do not run tests
   or builds on the coordinator workstation. Use one Cargo worker initially.
3. Run all applicable container combinations once before batching product fixes.
   Use fresh independent fixtures, continue independent cases after failures,
   and distinguish native-tool control, staged binary and installed package.
4. Exercise create/encrypt/format/open/mount read-write; copy the downloaded
   Bitcoin, Ethereum and Transformer PDFs plus text; close normally; reopen,
   compare hashes, append/create another file, close/reopen and compare again.
   Verify a real mount before writing. Preserve complete logs and failed fixtures.
   Run password/keyfile and AES/Serpent/Twofish variations where implemented;
   cover explicit/Auto open, wrong credentials, duplicate and busy close using
   existing conformance suites. Never weaken fixed encryption profiles to pass.
5. Check journaling, CoW, ACL, permissions and ownership briefly where relevant.
   Record yes/no with the command and observation, or unknown/not-applicable
   when unavailable. Feature depth is intentionally small, not an extensive
   filesystem audit. Read-only/no-format/no-mount are limitations, not features.
6. After containers, exercise the verified scratch disks through applicable
   native encryption/filesystem lifecycles. On macOS sample a few representative
   USB cases. If Lurker lacks a native-device adapter, test native controls and
   fix the adapter where feasible; do not label a native control a Lurker pass.
7. Batch compatible fixes, commit each atomic change promptly with appropriate
   checks, build once per changed input batch, rerun affected cases and then
   perform a final complete validation against the frozen candidate. Run
   cargo check --workspace and cargo test --workspace on remote targets;
   native runtime evidence must match the actual tested revision/artifact.
8. Reconcile every planned cell and case to current receipts; record incomplete,
   failed, unavailable and upstream limitations without turning them into passes.
   Inspect cleanup, retain required evidence and report remaining work honestly.

Ordinary VM reboot persistence is part of the development runbook; batch owned
fixtures per reboot when capacity permits. Buran uses another close/reopen and
records OS reboot unexercised. Do not expand into crash/power-loss or deep
application workloads unless a discovered issue requires them. Before any
database workload read /home/adjutant/.codex/DATABASE.md.

## Evidence, commits and handoffs

Local private evidence root: target/goal-validation/native-fresh-20261127/.
Use payload/, hosts/HOST/{inventory,updates,build,cases,device,validation,logs}/
and a per-host checkpoint.json. Remote roots are
/tmp/lurker-native-fresh-20261127-HOST (small source/log files) and an identified
persistent per-host cache/fixture root where /tmp lacks space. Evidence and test
payloads stay uncommitted. Track concise reports and handoffs in this directory.

Each case receipt identifies host, OS/kernel, backend, filesystem and role
(inner/backing), source kind, native/product route, source revision, binary hash,
tool versions, operations, expected/actual hashes, quick feature observations,
outcome, log paths and cleanup. Use passed/failed/blocked/unavailable/not-applicable
honestly. A missing receipt is incomplete. Keep manifests/checkpoints atomic.

Freeze harness inputs from a named commit (for example with `git show REV:path`)
before uploading them; other host owners may be editing the shared worktree.
Record the executed script hash as well as its base revision. Payload hashes
alone do not prove filesystem identity: capture native mount and backing-store
types. Reject mismatches, and retain driver/superblock evidence for generic FUSE
or UFS-family labels. Bound payload I/O as well as CLI subprocesses; retain
stalled worker identities and failed storage for scoped normal recovery.

Freeze baseline from the commit containing these handoffs; capture it before
archiving. Coordinator owns shared docs and shared Rust dispatch/model/profiles.
Host ownership is in HOSTS.md. Linux shared changes belong to Arch's owner;
other Linux owners submit concise proposed changes and avoid the same file.
No further delegation is needed. Read only BRIEF.md and your HOSTS.md section
plus required project directives; avoid dumping old reports or full build logs.
Return concise milestones with live process/run handles and exact next action.

All agents share one Git worktree. Inspect index/worktree before editing/staging,
stage only owned paths, and use an exclusive flock on
/tmp/lurker-native-fresh-20261127-git.lock around index inspection, staging and
commit. Do not commit if unrelated staged files would be included. Review diff
and applicable checks before each descriptive atomic commit. Never amend/squash.
Coordinate shared changes; do not push an unvalidated mixed candidate.
