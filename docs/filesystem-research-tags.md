# OS / Filesystem Research tags

## Planning summary notation

The README and planning summary use the following compact notation. The older detailed per-solution survey retains its independent format/mount tags below.

**Legend**

- **✅** — Create + R/W: create/format, reopen and mount both physical partitions and file containers for normal file reads and writes.
- **✅ Beta** — The same Create + R/W workflow, fully or largely implemented, with experimental maturity and awaiting production maturity.
- **⚠️ Limited Support** — Create + R/W exists, but important features are limited or implementation completeness remains unresolved, as in note ². This groups limited implementations, including those previously marked beta or legacy; their individual qualifications remain in the research notes.
- **❗ + capability** — Creation or normal mounted R/W is missing or unconfirmed. **R/O** means mounted read-only access, whether or not formatting is available.
- **—** — Unsupported for this use case: no qualifying mounted route found, including creation-only tools.
- **¹** — Mounted R/W exists; the proposed formatter-to-driver creation workflow remains unverified.
- **²** — Basic workflow exists; implementation completeness remains unresolved, rather than a confirmed missing feature.
- **³** — Requires working DragonFly FUSE support, omitted from the stock 6.4.2 kernel.

Beta is independent of feature completeness. Linux HAMMER2 physical-device reliability remains unqualified by its scratch-image tests. Windows OpenZFS is largely implemented; its remaining stream-renaming limitation does not prevent the core workflows.

This is filesystem capability research based on documentation/source inspection, not a claim that these combinations are integrated or runtime-qualified in Lurker.

This planning summary follows the stated use case: format a decrypted physical partition or file container, then reopen and mount it for normal application file access. Tools run on the host OS and are freely obtainable by individual users; virtual machines, WSL, paid routes and extract–edit–reinsert workflows are excluded. Native and userspace mounts count equally. Separate host-installed formatters and mount drivers may be combined, but an unverified combination is not a confirmed creation route.

## Detailed survey notation

Each cell contains only tags, with a separate entry for every researched
offering. Keep alternatives; do not consolidate them. All status tags are
prefixes, in the order of the four independent dimensions below:

`[Paid] [Beta] [Partial or Partial*] [Outdated] Format Mount Access [(product number)]`

Omit default status tags; never write `None`, `Free`, `Production`, or
`Current` in a cell. List solution names in the notes in the same order as
the cell.
Use Markdown continuation rows for multiple entries; no HTML is required.

Assess formatting and mounting independently for partitions and images. An
image attached as a virtual block device counts as image support.

Each entry must belong to an existing solution or documented tool combination.
The strongest target/access offering is `F M R/W`; that alone does not establish
feature completeness. Assess each offering separately and preserve its sources.

## Research bar

- Cover Linux, macOS, FreeBSD, OpenBSD, NetBSD, DragonFly BSD, and Windows.
- Cover ext4, Btrfs, XFS, APFS, UFS1 / FFSv1, UFS2 / FFSv2, HAMMER2, NTFS,
  exFAT, and ZFS / OpenZFS.
- Include solutions any individual can obtain through the OS, a public
  open-source or freeware download, or a retail purchase. OEM-only products
  and WSL are outside the scope.
- Count mount access when ordinary apps can open files directly through an OS
  mount point or drive letter and save changes when writes are supported.
  Built-in drivers and frameworks such as FUSE, FSKit, WinFsp, and Dokan
  qualify on the same basis. Exclude extract–edit–reinsert workflows.
- Count independently supported formatting even when mounting is unavailable.
  Use `NM` when mount support is known to be absent in the stated setup.

## Status prefixes

| Dimension | No tag means | Explicit prefix |
| --- | --- | --- |
| Availability | Free: OS-provided, freeware or freely obtainable open source. | `Paid`: commercial offering available for individual purchase. |
| Maturity | Production. | `Beta`: usable experimental or early-development implementation. |
| Completeness | Fully or largely implemented, based on the feature assessment. | `Partial`: confirmed important missing features. `Partial*`: completeness cannot be determined. |
| Maintenance | Current. | `Outdated`: legacy or outdated implementation. |

The dimensions are independent. Paid offerings can be experimental, incomplete
or outdated; a production implementation can be partial. Maturity describes
stability/reliability, while completeness describes implemented features.
Number multiple paid products and name them in the solution notes. Optional
donations or paid support do not make a freely obtainable offering `Paid`.

## Format / Create

| Tag | Meaning |
| --- | --- |
| `FP` | Format a physical partition. |
| `FI` | Format a file-backed image. |
| `F` | Format both targets. Short for `FP+FI`. |
| `NF` | No formatting support. |

## Mount

| Tag | Meaning |
| --- | --- |
| `MP` | Mount a physical partition. |
| `MI` | Mount a file-backed image. |
| `M` | Mount both targets. Short for `MP+MI`. |
| `NM` | No mount support. Omit the mount access tag. |

## Mount access

| Tag | Meaning |
| --- | --- |
| `R` | Read-only. |
| `LR` | Limited reads, no writes. |
| `R/W` | Normal reads and writes. |
| `R/LW` | Normal reads, limited writes. |
| `LR/W` | Limited reads, normal writes. |
| `LR/LW` | Limited reads and writes. |

Use `F` and `M` only when both targets are confirmed. If a target remains
unverified, use the confirmed tag and note the uncertainty. If neither target
is confirmed, record the uncertainty in a note. Reserve `NF` and `NM` for known
absence of support. Use `—` when no qualifying solution has been found.

Note specific limits such as `overwrite only`, required OS builds or kernel
modules, and supported on-disk features in the notes for that solution.

## Implementation completeness

Prefix `Partial` when an implementation lacks important filesystem features.
Prefix `Partial*` when an AI agent cannot determine whether the implementation
is fully/largely implemented or has important gaps. Unknown completeness must
never silently imply full/largely implemented support. Keep the existing
target/access tags: `R/W` describes ordinary file reads/writes, not complete
implementation of every filesystem
feature. `R/LW` describes restrictions such as overwrite-only access. Either
can also receive `Partial` for significant missing implementation features.

Compare with the filesystem's maintained reference implementation, using this
standard benchmark rather than counting every optional feature equally:

- Ordinary files and directories: create, read, overwrite, extend, truncate,
  rename, delete, large files, Unicode and normal case behavior.
- Metadata and semantics where the format provides them: timestamps,
  ownership/permissions, ACLs, extended attributes/streams, links and sparse files.
- Integrity and recovery where the format provides them: checksums,
  journal/log replay and transactional writes or copy-on-write consistency.
- Important filesystem-specific features and common on-disk profiles below.

| Filesystem | Important features beyond ordinary file operations |
| --- | --- |
| ext4 | Extents, 64-bit and metadata-checksummed layouts, journal handling, sparse files, ACLs/xattrs. |
| Btrfs | Checksums, log recovery, compression, subvolumes/snapshots, reflinks and supported multi-device profiles. |
| XFS | Current metadata layouts, log recovery, sparse files, ACLs/xattrs, reflinks. |
| APFS | Copy-on-write, clones/snapshots, compression, encrypted data volumes, metadata and accepted container layouts. |
| UFS1 / FFSv1; UFS2 / FFSv2 | The particular BSD variant's metadata, ACLs/xattrs and recovery mechanisms where implemented; no universal cross-BSD profile is assumed. |
| HAMMER2 | Local-volume checksums, copy-on-write consistency, compression, deduplication, PFS/snapshots and supported volume layouts. |
| NTFS | Metadata/permissions, named streams, links/reparse points, sparse/compressed files and transactional recovery. |
| exFAT | Normal allocation, directories, names, timestamps and large-file operations; no requirement for features absent from exFAT itself. |
| ZFS / OpenZFS | Pool/dataset operations, integrity/recovery, metadata, snapshots/clones, compression and native encryption. |

Assess features relevant to the advertised operation. Read-only readers need
not create files or snapshots, but should expose supported existing data and
metadata correctly. Formatters need not mount; `NM` already expresses that.
Important layout exclusions or inability to recover a normal supported volume
can still justify `Partial`. Optional/exotic features alone need not: largely
implemented support needs no completeness prefix. For example, missing EFS
plaintext decryption alone does not make an otherwise broad NTFS reader `Partial`.

Completeness is independent of the other status dimensions, implementation
location and mount transport. Native and paid drivers can be partial. Read-only mode, age,
experimental maturity, performance, an installation prerequisite or lack of
formatting alone does not justify it. A feature absent from the format's own
design is not an implementation gap. Normal release-to-release feature-flag
compatibility is a condition to document, not automatically `Partial`.

Record every `Partial` reason with an upstream source in the
[implementation assessment](filesystem-implementation-assessment.md).
For every `Partial*`, record the specific unresolved features or missing
evidence, the sources checked, and why completeness could not be determined.
The asterisk means uncertainty, not a confirmed missing feature. Use one of
`Partial` or `Partial*`, never both: a confirmed important gap is sufficient
for `Partial`, even when other features remain unverified. Remove the asterisk
only after evidence establishes either `Partial` or full/largely implemented
support. Do not infer a vendor limitation from silence. Keep research
judgments separate from runtime validation.

## Examples

- `F M R/W` combines formatting and normal read/write mounts on both targets.
- `F MI R/LW` formats both targets and mounts images with limited writes.
- `FI NM` formats images and has no mount support.
- `NF MP R · MI R/W` gives each mount target its own access tag on the same line.
- `Partial F M R/W` supports ordinary reads/writes but lacks important features.
- `Beta Partial NF M R` is experimental, read-only and additionally incomplete.
- `Paid Partial FP MP R/W (1)` is a paid implementation with confirmed gaps.
- `Paid Partial* NF MP R/W (1)` is paid, with completeness undetermined and explained in the notes.
- `Beta Partial Outdated NF M R/W` combines three independent status prefixes.
