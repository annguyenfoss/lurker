# Filesystem integration complexity audit

Audit date: 2026-11-26. Source baseline:
`bf8dfe194fc273669cb2652931f071cd6095c328`.

This audit records the code review and engineering assessment requested after
the filesystem qualification round. It asks which integrations need more than
selecting an installed formatter and mount command, and whether the additional
code is justified. It covers Linux's shared implementation for all six
distributions, macOS, FreeBSD, NetBSD, OpenBSD and DragonFly BSD. Windows is
outside this audit.

This was a source and existing-report review. No new native tests, installations,
driver comparisons or simplification experiments were run. References to earlier
passes or failures retain their original evidence and scope. The audit does not
change the README promise matrix or qualify additional filesystem capabilities.

## Finding

Most native filesystem integrations should remain small adapters around existing
tools. The current Linux privileged implementation already exposes a normal
decrypted device and gives it to the formatter and mount command. Most ordinary
BSD filesystem dispatch follows the same model.

There are justified exceptions: partition geometry, driver-compatible format
options, ZFS pool management, and particular provider interface defects. Some
larger mechanisms are consequences of Lurker's workflow or the selected provider.
Their existence does not establish that they are the simplest implementation.

The required behaviour and the current implementation must be judged separately.
A needed ownership check, for example, does not justify every registry field,
parser or recovery branch around it. This audit does not certify all existing
lifecycle machinery as minimal or necessary.

## Product constraints and terminology

- The goal is **Create + Read/Write**, with one mount action and one unmount
  action. Read-only access is a capability shortcoming, not a desired mode or
  an independent required test. Retaining detection of a write-capability loss
  does not introduce a selectable access mode.
- Only software free of charge is eligible. Prefer one adequate tool or suite;
  combine tools only when another efficiently covers a material capability gap.
- Ordinary create, mount, write, normal close, reopen, verify, append/add-file
  and reopen establishes representative use. This audit adds no exhaustive
  filesystem-feature qualification requirement.
- Kernel/FSKit drivers and local FUSE processes are distinguished below from
  NFS/SMB bridges and filesystem VMs. A local FUSE driver is not a kernel
  filesystem driver, but it also does not inherently require a network share
  or a VM.
- Encryption device acquisition, privilege handling, atomic creation and safe
  teardown are shared storage responsibilities. They must not be attributed
  repeatedly to each new filesystem as new filesystem-engine development.

## What the code already does

For an ordinary privileged Linux filesystem:

```text
Create: open encryption -> format decrypted device -> initialize new root -> close
Mount:  open encryption -> identify filesystem -> mount decrypted device
Close:  unmount filesystem -> close encryption
```

Creation passes `/dev/mapper/lurker_...` directly to `format_filesystem`.
Opening an existing container does not format it. All six Linux distributions
use these same functions; installed package/kernel differences do not require
six separate filesystem implementations. See [S1] and [S2].

Linux's separate login-user route deliberately avoids kernel mappings and uses
a VeraCrypt decrypted regular image plus an LKL filesystem helper. Its process
and shutdown protocol are costs of that selected unprivileged architecture,
not evidence that ordinary kernel filesystem mounting needs an LKL engine.
See [S15].

| Ordinary route | Filesystem-specific work in the current code |
| --- | --- |
| Linux / ext4, Btrfs, XFS | Select formatter and arguments; initialize a private root owned by the invoking user. Btrfs adds `compress=zstd` when mounting. |
| Linux / NTFS, exFAT | Select formatter and mount driver; set synthetic ownership and private permissions. NTFS explicitly selects NTFS-3G. |
| FreeBSD / UFS1, UFS2, ext4, exFAT | Select `newfs`, `mkfs.ext4` or `mkexfatfs`, then the appropriate mount command. |
| BSD / ordinary UFS/FFS mounts | Select version and OS mount options. NetBSD requests WAPBL journaling and checks it is active; OpenBSD uses synchronous writes. |
| macOS / Apple disk images with APFS or exFAT | Give the filesystem name to `hdiutil create`; use attach/detach for the image lifecycle. |

Linux root initialization has small formatter-specific differences: Btrfs uses a
seed directory, XFS a prototype, and ext4 a temporary mount because supported
older e2fsprogs lacks `root_perms`. Existing imported filesystem ownership is
not changed. These are permission initialization techniques, not new filesystem
engines. See [S1], [S2], [S3] and [S4].

## Assessment of the exceptions

"Necessary with this provider" means the current tool interface requires the
adaptation. It does not establish that every eligible alternative needs it.

| Case or mechanism | Concrete reason and current behaviour | Judgment | Evidence |
| --- | --- | --- | --- |
| OpenBSD / NTFS and exFAT partition layout | Their formatters overwrite the label area with an offset-zero partition. Creation places partition `a` at sector 64, verifies its geometry/type, then formats the block node. | Necessary with the current tools; a small disk-layout adapter. The earlier report records the actual label failure and correction. | [S5], [R1] |
| NetBSD / non-UFS inner partitions | Create a disklabel partition at sector 2048 with an appropriate type; choose raw or block nodes for each tool. Non-FFS CGD creation also selects the existing disklabel-verification route. | Necessary for the current CGD/layout/tool combination; share the layout code across filesystems. | [S6], [R2] |
| NetBSD / ext4 format profile | Disable `64bit`, `metadata_csum` and `orphan_file` for the selected fuse-ext2 driver. | Necessary compatibility configuration; only a few formatter arguments. | [S6] |
| NetBSD / HAMMER2 size | The device ioctl reports whole-disk geometry even for a partition. Calculate and supply a size bounded to the inner partition. | Necessary bounds protection with this formatter interface; a small calculation and argument. | [S6] |
| Linux / UFS1 and UFS2 | Select the FreeBSD-compatible formatter, detect the UFS version, select the driver, and reject an unexpected mount without write support. | Necessary compatibility adapter. Does not implement UFS in Lurker. A formatter binary alone does not establish a compatible writer. | [S1], [S2] |
| New filesystem root ownership | A filesystem formatted as root must still be privately usable by the invoking user. Initialize only a newly created root; use mount options for synthetic owners. | Necessary behaviour. Prefer shared initialization with small formatter-specific arguments. | [S1], [S7] |
| Mount identity and ordered close | FUSE may publish a generic source name. Prove which mount belongs to the decrypted device; stop if normal unmount fails; close filesystem state before encryption. | Necessary behaviour. Exact fields, duplicated parsers and recovery branches require their own justification. | [S5], [S6], [S7] |
| DragonFly / HAMMER2 direct syscall | `mount_hammer2_local` says the normal helper starts a cluster service that can inherit the registry lock. It calls the native mount syscall with local-PFS arguments instead. | Justified current workaround; not established as the minimum solution. Check helper options and descriptor inheritance before assuming Rust must maintain this native argument structure. No simpler alternative was validated in this audit. | [S8] |
| ZFS / Linux, FreeBSD, NetBSD, macOS | Create/import a pool, identify its device, mount the dataset, then export before closing encryption. Scope operations to the container's pool and preserve other mounts. | Substantial additional lifecycle integration is necessary. The current amount of parsing, duplication and recovery machinery is not thereby certified minimal. | [S1], [S3], [S6], [S9], [R2] |
| macOS / NTFS corrective remount | Let VeraCrypt mount first. If it succeeds through Apple's driver without write support, unmount that filesystem and use TT NTFS FSKit on the same decrypted device. | Avoidable complexity in the current orchestration. Explicit driver selection after opening encryption could remove this corrective path; that replacement still needs validation. | [S10], [S11] |
| macOS / UFS2 formatter staging | UFS2Tool truncates its output. Format a separate blank image, initialize its root, verify identity/size, copy it into the decrypted view without truncating that view, sync and close. | Necessary with the selected formatter. A compatible formatter accepting an existing device/image could remove this workaround; an adequate replacement was not established here. | [S11], [S12] |
| macOS / UFS2 provider patches | Patches fix flush/fsync, indirect-block initialization, Darwin/macFUSE compatibility, rename request layout and a formatter backup superblock. Unsupported cross-parent directory moves are refused. | Actual provider compatibility/correctness work, not wrapper configuration. It is a maintenance cost of the selected tools; it does not establish complete UFS2 semantics. | [S12], [R3] |
| FreeBSD / APFS reader input | The selected reader route requires a regular decrypted image. The adapter finds and verifies VeraCrypt's backing image; GELI's raw provider is refused. | Necessary input adaptation for this reader. A reader accepting the block device would avoid it. Reading remains a shortcoming against Create + Read/Write. | [S3] |
| NFS/SMB/VM filesystem routes | macOS Btrfs uses a supervised Linux guest; ext4 uses NFS; XFS uses SMB. They require transport-specific startup, identity and close handling. | Large architectural tradeoffs, not inherent native-filesystem requirements. Justify them by a demonstrated gap in adequate free local alternatives. Removing them alone does not provide a replacement. | [S11], [S13], [R3] |

Native APFS encryption is a separate case: its createContainer/addVolume,
unlockVolume and lockVolume sequence manages APFS-provided encryption. It is
required for that selected encryption backend, not for an ordinary APFS
filesystem inside an already encrypted Apple or VeraCrypt image. See [S14].

The table classifies filesystem adapters, not the entire encryption transport.
In particular, a local macOS NTFS, ZFS or UFS2 driver does not make the current
VeraCrypt path network-free: its decrypted-image underlay uses FUSE-T SMB.
See [R3] and the transport contract in [the codebase directive](../DIRECTIVES/LLM/CODEBASE.md).

## Simplification candidates, not validated changes

### macOS explicit filesystem selection

The clearest candidate is the mount orchestration in [S10] and [S11]. Today it
first lets VeraCrypt attempt the filesystem mount. Matching either of two
human-readable error strings activates the optional-filesystem path, which
opens encryption without mounting a filesystem and chooses an adapter. A
successful NTFS mount without write support triggers a separate replacement
path. Both error-driven fallback and corrective remount add branches.

Evaluate a single sequence:

```text
Open encryption without mounting the filesystem
-> identify filesystem
-> select its supported driver
-> mount once
```

This is an engineering recommendation, not a proven drop-in replacement.
Validate it against working routes before changing them. Preserve native device
authorization, source identity, normal busy-close behaviour and any required
provider process cleanup. Compare the resulting total code rather than assuming
that moving all driver selection into Lurker necessarily makes it smaller.

### Shared routines and provider-specific work

- Keep ordinary formatter/mount differences in explicit small command builders.
  Do not add a generic plugin framework, external profile system, or one full
  workflow per filesystem or Linux distribution to solve these cases.
- Review repeated ownership, mount readiness and source-identity work for reuse.
  This audit identified common responsibilities, not a proven line-by-line
  redundancy or an approved registry rewrite.
- For DragonFly HAMMER2, check whether helper options or preventing descriptor
  inheritance eliminates the need for the direct syscall. Retain the working
  implementation until a simpler route is shown to work.
- For macOS UFS2, evaluate an alternative formatter or a small upstream interface
  fix before accepting blank-image copying as permanent. Account for the cost
  of maintaining that provider change; no superior formatter was found here.
- For bridges/VMs, compare capability benefit and total maintenance cost against
  free local alternatives. The audit does not authorize removing a working
  route or claim that an equivalent local writer has already been demonstrated.

## Review criteria for additional complexity

For a proposed mechanism or a later simplification audit, record:

1. The concrete failure it prevents, with a code reference and, where available,
   an existing reproduction or native receipt. A code comment is an explanation,
   not an independently reproduced result.
2. Whether an existing tool argument or a small shared routine already handles
   the requirement.
3. Whether the requirement comes from the OS/filesystem, the selected provider,
   or Lurker's own workflow.
4. The simpler alternative considered, its actual validation status, and what
   behaviour would be lost by removing the mechanism.

Ordinary native support should default to a shared decrypted-device lifecycle
and small command adapters. Larger exceptions need specific evidence. Retaining
working safeguards does not prove that every surrounding abstraction is needed.

## Source and report index

Source references describe the baseline commit above; function names are given
so readers can locate the evidence after line numbers change. Reports are
historical records, and later receipts supersede earlier attempts where noted.

- [S1]: `luks_create_device`, `mount_open_mapper`, `format_filesystem`,
  `initialize_native_root`, `mount_zfs`, `export_zfs`, `dmcrypt_umount_device`
  in [Linux workflow](../crates/lurker-core/src/linux_workflow.rs).
- [S2]: `linux_formatter`, `linux_format_args` in
  [tool selection](../crates/lurker-core/src/system.rs).
- [S3]: `validate`, `format`, `mount`, `close`, `apfs_input` in
  [FreeBSD filesystem adapters](../crates/lurker-core/src/bsd/freebsd/filesystems.rs).
- [S4]: `macos_apple_create_args`, `macos_filesystem` in
  [workflow profiles](../crates/lurker-core/src/workflow.rs).
- [S5]: `make_selected_filesystem`, `mount_filesystem`, `fuse_holds_mapping` in
  [OpenBSD filesystem adapters](../crates/lurker-core/src/bsd/openbsd/filesystems.rs).
- [S6]: `label_partition`, `make_selected_filesystem`, `mount_filesystem`,
  `matches_filesystem_mount`, `close_filesystem` in
  [NetBSD filesystem adapters](../crates/lurker-core/src/bsd/netbsd/filesystems.rs).
- [S7]: `initialize_new_filesystem_owner`, `require_write_journal`, `cleanup` in
  [shared BSD lifecycle](../crates/lurker-core/src/bsd.rs), and the
  [small filesystem dispatch interface](../crates/lurker-core/src/bsd/filesystems.rs).
- [S8]: `mount_hammer2_local` in
  [DragonFly VeraCrypt integration](../crates/lurker-core/src/bsd/dragonfly/veracrypt.rs).
- [S9]: `create`, `mount`, `verify_pool`, `export`, `close` in
  [macOS ZFS integration](../crates/lurker-core/src/macos/zfs.rs).
- [S10]: `mount` in [macOS workflow](../crates/lurker-core/src/macos.rs).
- [S11]: `open_image`, `format_ufs2`, `start_native_ntfs`, `replace_native_ntfs`,
  `start`, `mount_missing`, `close_session` in
  [macOS filesystem adapters](../crates/lurker-core/src/macos/filesystems.rs).
- [S12]: [macOS UFS2 patch recipe, pins and input restriction](../native/macos/fuse-ufs.md)
  and the patch files named there.
- [S13]: [macOS filesystem VM integration](../crates/lurker-core/src/macos/vm_filesystem.rs).
- [S14]: `initialize_partition`, `add_volume`, `mount`, `unmount` in
  [native APFS encryption](../crates/lurker-core/src/macos/apfs.rs).
- [S15]: `create`, `start_filesystem`, `unmount` in
  [Linux login-user lifecycle](../crates/lurker-core/src/userspace.rs), with
  the architectural contract in [the codebase directive](../DIRECTIVES/LLM/CODEBASE.md).
- [R1]: [OpenBSD results](../DIRECTIVES/PLAN/FILESYSTEM_ROUND/OPENBSD_RESULTS.md),
  especially the implementation/label correction narrative.
- [R2]: [NetBSD results](../DIRECTIVES/PLAN/FILESYSTEM_ROUND/NETBSD_RESULTS.md),
  especially partition verification, mount identities and restricted ZFS discovery.
- [R3]: [macOS retest results](../DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md),
  including the later UFS2 patches and actual encrypted lifecycle qualification.

[S1]: ../crates/lurker-core/src/linux_workflow.rs
[S2]: ../crates/lurker-core/src/system.rs
[S3]: ../crates/lurker-core/src/bsd/freebsd/filesystems.rs
[S4]: ../crates/lurker-core/src/workflow.rs
[S5]: ../crates/lurker-core/src/bsd/openbsd/filesystems.rs
[S6]: ../crates/lurker-core/src/bsd/netbsd/filesystems.rs
[S7]: ../crates/lurker-core/src/bsd.rs
[S8]: ../crates/lurker-core/src/bsd/dragonfly/veracrypt.rs
[S9]: ../crates/lurker-core/src/macos/zfs.rs
[S10]: ../crates/lurker-core/src/macos.rs
[S11]: ../crates/lurker-core/src/macos/filesystems.rs
[S12]: ../native/macos/fuse-ufs.md
[S13]: ../crates/lurker-core/src/macos/vm_filesystem.rs
[S14]: ../crates/lurker-core/src/macos/apfs.rs
[S15]: ../crates/lurker-core/src/userspace.rs
[R1]: ../DIRECTIVES/PLAN/FILESYSTEM_ROUND/OPENBSD_RESULTS.md
[R2]: ../DIRECTIVES/PLAN/FILESYSTEM_ROUND/NETBSD_RESULTS.md
[R3]: ../DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md
