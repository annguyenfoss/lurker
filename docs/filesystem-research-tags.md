# OS / Filesystem Research tags

Each cell contains only tags, with a separate entry for every researched
offering. Keep alternatives within the same prefix group (None, Beta,
Outdated, Paid); do not consolidate them. Write the prefix, formatting tag,
mount tag, mount access, any paid-product number, and finally `Partial` when
applicable. List solution names in the notes in the same order as the cell.
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

## Prefixes

| Prefix | Meaning |
| --- | --- |
| None | Production-grade support provided by the OS or publicly available for free. |
| `Beta` | Usable experimental or early-development support. |
| `Outdated` | Legacy or outdated support. |
| `Paid` | Commercial product available for individual purchase. Number multiple products and name them in the solution notes. |

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

Append `Partial` at the very end when an implementation lacks important
filesystem features. Keep the existing target/access tags: `R/W` describes
ordinary file reads/writes, not complete implementation of every filesystem
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
implemented support needs no suffix. For example, missing EFS plaintext
decryption alone does not make an otherwise broad NTFS reader `Partial`.

`Partial` is independent of prefix, price, implementation location and mount
transport. Native and paid drivers can be partial. Read-only mode, age,
experimental maturity, performance, an installation prerequisite or lack of
formatting alone does not justify it. A feature absent from the format's own
design is not an implementation gap. Normal release-to-release feature-flag
compatibility is a condition to document, not automatically `Partial`.

Record every `Partial` reason with an upstream source in the
[implementation assessment](filesystem-implementation-assessment.md). Record
insufficiently documented coverage explicitly there; absence of the suffix
must not be represented as verified completeness. Do not invent a new matrix
tag for uncertainty or infer a vendor limitation from silence. Keep research
judgments separate from runtime validation.

## Examples

- `F M R/W` combines formatting and normal read/write mounts on both targets.
- `F MI R/LW` formats both targets and mounts images with limited writes.
- `FI NM` formats images and has no mount support.
- `NF MP R · MI R/W` gives each mount target its own access tag on the same line.
- `F M R/W Partial` supports ordinary reads/writes but lacks important features.
- `Beta NF M R Partial` is experimental, read-only and additionally incomplete.
- `Paid FP MP R/W (1) Partial` puts the suffix after the paid-product number.
