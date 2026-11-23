# Writable filesystem integration matrix

This proposal covers filesystems for data partitions, external drives, and
block devices exposed by VeraCrypt and other disk containers. It prioritizes
the main native filesystem on each OS, followed by established formats with
writable support across multiple OSes. See the
[OS filesystem survey](filesystem-support-matrix.md) for driver sources,
version restrictions, and the broader inventory.

**C** means create or format. **RW** means open and mount read/write.
**Native** uses host facilities. **Package** requires a separately installed
driver or tools. **Candidate** requires native qualification on the listed
driver and volume type. **Paid** means a supported commercial driver is
required. A dash leaves the OS/filesystem pair outside the proposed scope.

| Filesystem | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **ext4** | Native C/RW | Candidate paid C/RW | Candidate native RW for compatible profiles | — | — | — | Candidate free driver RW or paid RW |
| **Btrfs** | Native C/RW | — | — | — | — | — | Candidate WinBtrfs C/RW |
| **XFS** | Native C/RW | — | — | — | — | — | — |
| **APFS** | Candidate paid RW | Native C/RW | — | — | — | — | Candidate paid RW for supported volumes |
| **UFS2 / FFSv2** | — | — | Native UFS2 C/RW | Native FFSv2 C/RW | Native FFSv2 C/RW | — | — |
| **HAMMER2** | — | — | — | — | — | Native C/RW | — |
| **DragonFly UFS** | — | — | — | — | — | Native RW for existing volumes | — |
| **NTFS** | Native RW, formatter package C | Candidate paid RW | Candidate NTFS-3G RW | Candidate NTFS-3G RW | Candidate NTFS-3G RW | — | Native C/RW |
| **exFAT** | Native C/RW | Native C/RW | Package C/RW | Package C/RW | Package C/RW | Candidate C/RW once release FUSE works | Native C/RW |
| **OpenZFS** | Package pool and dataset workflow | Candidate package pool and dataset workflow | Native pool and dataset workflow | — | Candidate pool and dataset workflow | — | — |

## Integration priorities

1. **Native data volumes.** Use ext4 and Btrfs on Linux, APFS on macOS,
   UFS2 on FreeBSD, FFSv2 on OpenBSD and NetBSD, HAMMER2 on DragonFly BSD,
   and NTFS on Windows. Keep XFS as an established Linux choice and retain
   writable access to existing DragonFly UFS volumes. Qualify UFS2 and FFSv2
   on each OS with its own formatter defaults.
2. **Portable volumes.** Start with exFAT across Linux, macOS and Windows
   and the packaged drivers on FreeBSD, OpenBSD and NetBSD. DragonFly BSD
   6.4.2 omits the FUSE kernel module required for its exFAT driver. NTFS
   gives Windows and Linux a journaled alternative. WinBtrfs is the
   Windows route to qualify for existing Linux Btrfs containers.
3. **Additional writable imports.** Qualify compatible ext4 on FreeBSD
   against its journal and feature limitations. On Windows, evaluate
   Ext4Windows through WinFsp and Ext4Fsd as free ext4 candidates alongside
   the paid option. Verify current Linux formatter defaults, journal
   recovery, and writable mounts on the actual partition and unlocked
   container device. Keep third-party APFS, ext4, and NTFS drivers optional.
4. **OpenZFS pools.** Add FreeBSD and Linux first as a distinct single-device
   pool workflow. Qualify pool creation or import, dataset mount, and clean
   export before offering it. Use common feature flags when testing exchange.
   macOS and NetBSD are follow-up candidates with port-specific constraints.

For each selected route, verify formatting where offered, writable mount,
file creation and modification, clean unmount, and persistence after reopen.
For cross-OS support, create and write on each supported host in turn, then
reopen and verify the data on the other host. Where one host supports writable
import only, create on the formatting host and test edits in both directions.
Qualify physical-device and container-backed routes separately.

## Excluded from new integration

- **F2FS** meets the technical requirements for a writable filesystem. Its
  emphasis on NAND flash, including SD cards, eMMC, and SSDs, makes it a
  lower priority for this general-purpose scope. Also exclude **bcachefs**,
  **ReFS**, and **FAT12/16/32** as requested.
- **HFS+, ext2/ext3, HAMMER1, JFS, and ReiserFS** are lower-priority legacy
  choices. The selected filesystems cover their typical new-volume use cases.
- **BSD LFS, NILFS2, UDF, and the specialty flash and zoned filesystems** have
  narrower workloads. Dedicated cluster and SAN filesystems such as
  **GFS2, OCFS2, VxFS, StorNext, and IBM Storage Scale** carry separate
  deployment and management requirements.
- **ISO 9660, SquashFS, EROFS**, and other read-only formats or reader-only
  driver paths are outside the writable-volume scope.
