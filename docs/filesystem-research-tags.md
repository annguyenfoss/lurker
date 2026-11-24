# OS / Filesystem Research tags

Use one line per solution in each cell. Write the prefix, formatting tag,
mount tag, and mount access in that order. Assess formatting and mounting
independently for partitions and images. An image attached as a virtual block
device counts as image support.

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
| `Paid` | Commercial product available for individual purchase. When multiple paid solutions appear in a cell, append `(1)`, `(2)`, and so on, and identify the products in the legend. |

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
modules, and supported on-disk features beside the affected line.

## Examples

- `F M R/W` combines formatting and normal read/write mounts on both targets.
- `F MI R/LW` formats both targets and mounts images with limited writes.
- `FI NM` formats images and has no mount support.
- `NF MP R · MI R/W` gives each mount target its own access tag on the same line.
