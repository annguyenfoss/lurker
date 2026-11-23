# File system support by operating system

Survey date 2026-11-23. The matrix covers local disks, partitions, and
file-backed block devices on Linux, macOS, FreeBSD, OpenBSD, NetBSD, DragonFly
BSD, and Windows. It includes maintained native formats, optional open-source
drivers, experimental ports, commercial tools, and compatibility formats.
A separate inventory records specialized and legacy paths.

The [filesystem integration matrix](filesystem-integration-matrix.md) selects
writable formats from this survey for Lurker.

**exFAT is the modern interchange choice.** Linux, macOS, and Windows have
native read/write drivers. FreeBSD, NetBSD, and OpenBSD have packaged exFAT
drivers. DragonFly BSD has packages too, but stock 6.4.2 omits the kernel FUSE
module they need. **FAT32 is still the broadly available base-system fallback**
and cannot hold an individual file of 4 GiB or larger. Linux's `vfat` is the
driver name for FAT volumes with long file names, including FAT32. It uses the
same on-disk FAT format. For native daily use, the strongest candidates are
ext4, Btrfs, and XFS on Linux, APFS on macOS, NTFS on Windows, UFS2 on FreeBSD,
FFSv2 on NetBSD and OpenBSD, and HAMMER2 or UFS on DragonFly BSD.
[L1, L5, M1, W1, W2, F1, N1, O1, D1–D3]

## How to read the matrix

- **Native** uses an OS-supplied filesystem driver. Linux may need distro
  userspace tools to format, even with the native driver.
- **Package** uses a separately installed open-source driver, kernel module,
  or FUSE package. **Experimental** identifies a development, preview,
  restricted, or unqualified port. **Paid** identifies a commercial path.
- **C** means a formatter or pool creator exists. **RW** means read/write
  mounting. **R** means read-only mounting. **Image** means offline image
  creation. **Subset** identifies a feature, release, or media restriction.
  Multiple paths in a cell can have different capabilities.
- A dash means this survey identified no direct mount path for that host.
  Offline image builders and file recovery are identified where they apply.
  Windows entries cover drivers and tools that run directly on Windows.

The native inventory used Linux 7.2.15 on Arch, macOS 27.2.1 arm64, FreeBSD
15.1, NetBSD 11.1, OpenBSD 8.0, and DragonFly 6.4.2 x86-64. Windows 11 and
Windows Server claims use Microsoft documentation and named driver projects.
The SSH checks inventoried release IDs, installed formatters and mount
helpers, and DragonFly's module files and package catalog. No new filesystem
mount or write tests were run for this survey.

## OS and file system matrix

### General-purpose and interchange formats

| File system | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **ext4** | Native C/RW | Paid C/RW<br>Ext4Kit beta C/RW<br>xlinuxfs FSKit RW | Native RW subset<br>Package C | Native R subset<br>Package C | Native R subset<br>Package C<br>Experimental FUSE RW | Native R subset<br>Package C<br>RW on current ext4 profiles unverified | Open-source driver RW on supported profiles<br>Paid RW<br>Janus or FsRover R | L1, M2, M5, M8, F1, O2, N2, D4, W4–W5, W8–W9, X7 |
| **Btrfs** | Native C/RW | xlinuxfs FSKit RW<br>Experimental R via btrfs4mac | Package RW via old LKL port | — | — | — | WinBtrfs C/RW<br>Paid or FsRover R | L1, M8, F4, W5, W6, X6–X7 |
| **XFS** | Native C/RW | xlinuxfs FSKit RW<br>Experimental FUSE R/RW | Package RW via old LKL port<br>Separate package C | — | — | — | Paid R, vendor RW claims conflict<br>FsRover R | L1, M8, F4, W5, X6–X7 |
| **F2FS** | Native C/RW | — | — | — | — | — | FsRover R | L1, X7 |
| **bcachefs** | Package C/RW via upstream DKMS on current kernels | — | — | — | — | — | — | L6 |
| **ZFS / OpenZFS** | Package C/RW | Package C/RW, version and architecture dependent | Native C/RW | — | Native module C/RW, port-specific feature set | — | Experimental C/RW, release-candidate driver | Z1–Z4 |
| **APFS** | FUSE R, experimental kernel RW<br>Paid RW | Native C/RW | Experimental R port | — | — | — | Paid RW subset<br>Experimental WinFsp RW<br>FsRover R | M1, X1–X2, X7–X8 |
| **HFS+ / HFSX** | Native RW subset<br>Paid RW | Native C/RW, including journaled HFS+ | Package R | Experimental FUSE R | Native R | Package R requires working FUSE | Paid RW<br>FsRover R | L1, M1, M6, F1, N1, D2–D3, X3, X7 |
| **NTFS** | Native `ntfs3` or `ntfs` RW<br>Package C and NTFS-3G RW | Native R<br>Open-source FSKit RW, experimental C<br>NTFS-3G with FUSE-T RW<br>Paid C/RW | Package C/RW | Native R<br>Package C/RW | Native R and limited overwrite<br>Package C/RW | Native R<br>FUSE package C/RW requires working FUSE | Native C/RW | L2–L4, M3, M7, F1, O3, N3, D2–D3, W1 |
| **ReFS** | Experimental OpenRefs R<br>Paid RW on vendor-supported versions | Experimental FUSE R | Experimental FUSE R candidate | Experimental FUSE R candidate | Experimental FUSE R candidate | Experimental FUSE R candidate where FUSE works | Native C/RW on supported Windows edition and volume type | W1, W3, X4–X5 |
| **exFAT** | Native C/RW | Native C/RW<br>Legacy fuse-exfat C/RW with its documented runtime | Package C/RW via exFAT FUSE and utilities | Package C/RW via exFAT FUSE | Package C/RW via exFAT FUSE | Package C<br>RW requires working FUSE | Native C/RW | L5, M1, F1–F2, O3, N3, D2–D3, W1 |
| **FAT12/16/32** | Native C/RW | Native C/RW | Native C/RW | Native C/RW | Native C/RW | Native C/RW | Native C/RW | L1, M1, F1, O1, N1, D1, W1–W2 |
| **UFS1 / FFSv1** | Native R<br>Experimental writes for select variants | Experimental image creation | Native C/RW | Native C/RW | Native C/RW | Native C/RW for DragonFly's UFS format | UFS2Tool C and limited image RW via Dokan<br>FsRover R | U1–U3, F1, O1, N1, D1, X7 |
| **UFS2 / FFSv2** | Native R<br>Experimental writes for select variants | Experimental image creation | Native C/RW | Native C/RW | Native C/RW | — | UFS2Tool C and limited image RW via Dokan<br>FsRover R | U1–U3, F1, O1, N1, D1, X7 |
| **HAMMER2** | Experimental kernel C/RW<br>Userspace image tools | — | Package C/R on supported releases<br>Experimental RW | Experimental kernel port | Experimental kernel port | Native C/RW | — | D1, H1–H3 |
| **HAMMER** | — | — | — | — | — | Native C/RW, predecessor to HAMMER2 | — | D1 |
| **BSD LFS** | — | — | — | — | Native C/RW | — | — | N4 |
| **NILFS2** | Native C/RW with nilfs-utils | — | — | — | Native R | — | — | L1, N4 |
| **UDF** | Native RW subset<br>Package C | Native C and media-dependent mounting | Native R<br>Optional offline tools | Native R | Native C/RW, media and revision limits | Native R | Native C/RW, media and revision limits | L1, M6, F5, O4, N5, D5, W1–W2 |

### Specialized, legacy, and recovery formats

These rows matter for imports, read-only volumes, and specialized deployments.
They carry lower priority for new general-purpose volumes.

| File system | Linux | macOS | FreeBSD | OpenBSD | NetBSD | DragonFly BSD | Windows | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **ext2 / ext3** | Native C/RW, including through ext4 where compatible | Paid C/RW<br>FSKit projects vary | Native RW subset<br>Package C | Native ext2 C/RW<br>Compatible ext3 subset | Native ext2 C/RW<br>ext3 feature limits | Native ext2 RW<br>ext3 feature limits | Ext4Fsd or paid RW<br>FsRover R | L1, M2, M5, F1, O2, N2, D4, W5, W8, X7 |
| **ISO 9660 / Rock Ridge / Joliet** | Native R, image tools | Native R, image tools | Native R, `makefs` | Native R, `makefs` | Native R, `makefs` | Native R, `makefs` | Native R, image authoring supported separately | L1, M6, F5, O4, N5, D1, W7 |
| **EROFS / SquashFS** | Native R, package image builders | — | SquashFS offline tools | SquashFS offline tools | SquashFS offline tools | SquashFS offline tools | — | L1, S1 |
| **JFS** | Native RW when built, package C | — | — | — | — | — | FsRover R | L1, X7 |
| **GFS2 / OCFS2** | Native cluster drivers, package C/RW | — | — | — | — | — | — | L1, S2 |
| **VxFS** | Paid C/RW on supported Linux releases<br>Native `freevxfs` R subset | — | — | — | — | — | FsRover R | L1, S3, X7 |
| **Xsan / StorNext** | Paid StorNext client on qualified platforms | Xsan client, metadata controller and SAN dependent | — | — | — | — | Paid StorNext client on qualified platforms | S4 |
| **GPFS / IBM Storage Scale** | Paid enterprise cluster C/RW | — | — | — | — | — | Vendor- and release-specific client access | S5 |
| **FFS-style legacy UFS variants** | Native R for old/Sun/NextStep/Horizon variants, writes depend on variant and kernel | — | Select UFS variants | Select FFS variants | Select FFS variants | DragonFly UFS | Experimental UFS2Tool targets FreeBSD UFS variants | U1–U3, F1, O1, N1, D1 |
| **HFS classic** | Native legacy mount driver | Offline compatibility tools | Offline compatibility tools | — | — | Offline compatibility tools | FsRover R | L1, M6, S6, X7 |
| **Amiga FFS / AmigaDOS** | Native `affs` RW or R depending on format variant | — | — | — | Native `adosfs` R | — | FsRover R | L8, N1, X7 |
| **HPFS** | Native RW | — | — | — | — | Native R | FsRover R | L8, D5, X7 |
| **SGI EFS** | Native R | — | — | — | Native R | — | FsRover R | L1, N1, X7 |
| **System V Boot File System** | Native `bfs` driver, distro tools | — | — | — | Native `sysvbfs` with formatter | — | FsRover R | L1, N6, X7 |
| **V7 FS** | — | — | — | — | Native C/RW | — | — | N6 |
| **Acorn ADFS / FileCore** | Native R and limited overwrites on supported FileCore variants | — | — | — | Native R | — | FsRover R | L8, N6, X7 |
| **BeFS** | Native R | — | — | — | — | — | FsRover R | L8, X7 |
| **Minix FS** | Native C/RW with util-linux tools | — | — | — | — | — | FsRover R | L1, L8, X7 |
| **QNX4 / QNX6** | Native R | — | — | — | — | — | FsRover R | L1, L8, X7 |
| **OMFS** | Native RW | — | — | — | — | — | — | L8 |
| **ROMFS** | Native R, image creation tools | — | — | — | — | — | FsRover R | L1, L8, X7 |
| **CramFS** | Native R, package image tools | — | Offline tools | — | — | — | — | L1, S1 |
| **UBIFS / JFFS2 / CHFS** | Linux UBIFS and JFFS2 use flash/MTD interfaces | — | — | — | Native CHFS on supported flash devices | — | — | L1, N6 |
| **zonefs** | Native C/RW on supported zoned block devices | — | — | — | — | — | — | L9 |
| **FreeBSD tarfs** | Archive tools or userspace archive mounts | Archive tools or userspace archive mounts | Native R over tar archives | Archive tools | Archive tools | Archive tools | Archive tools | F5 |
| **ReiserFS** | Legacy Linux kernels, offline tools or FsRover R | — | Offline tools where packaged | — | — | — | FsRover R | L1, S6, X7 |
| **VMFS, FATX, FbFS, LynxFS, RedoxFS and SFS** | FsRover R | — | — | — | — | — | FsRover R | X7 |

## Findings by OS and filesystem

### Interchange and native defaults

- Use **exFAT** for large-file exchange where the required driver is present.
  FAT32 is the practical choice for an unchanged DragonFly 6.4.2 target or
  devices that require FAT. FAT and exFAT synthesize ownership and permissions
  at mount time and lack per-file POSIX metadata and metadata journaling.
  Include filename and case behavior, capacity limits, and recovery in an
  interchange test. [L5, M1, W2, D2]
- A filesystem family can contain incompatible feature profiles. Examples
  include ext4 extents and journal replay, XFS feature upgrades, ZFS pool
  feature flags, ReFS Windows releases, and FFSv2 with NetBSD WAPBL. Compare
  actual features and the formatter profile, especially when the recipient
  uses a port or compatibility driver. [L1, F1, O2, N2, N7, Z1, W3]

### Linux

- The current kernel documents two writable NTFS drivers, `ntfs3` and the
  newer `ntfs`. Distro kernels vary. NTFS-3G offers a separate userspace
  driver and `mkntfs`. The new kernel driver documents `ntfsprogs-plus`
  utilities. Treat the selected driver and formatter as separate capabilities.
  [L2–L4]
- `e2fsprogs`, `btrfs-progs`, `xfsprogs`, `f2fs-tools`,
  `exfatprogs`, and `dosfstools` provide the usual formatters for ext4,
  Btrfs, XFS, F2FS, exFAT, and FAT. F2FS targets flash-oriented storage
  and remains a native option for a compatible block device. bcachefs
  upstream supplies DKMS for current kernels after removal from Linux 6.18
  and later. Its maintainer describes the current release line as stable.
  Distribution support and exact kernel compatibility remain separate.
  [L1, L5–L7]
- Linux UFS support depends on the UFS variant and kernel build. The kernel
  configuration labels UFS write support experimental. A portable UFS
  import policy should start with read-only access and identify the variant.
  [U1]
- GFS2 and OCFS2 need explicit cluster configuration. [L1, S2]

### macOS

- `diskutil listFilesystems` on macOS 27.2.1 lists APFS, case-sensitive APFS,
  exFAT, FAT12/16/32, and HFS+ variants for normal disk formatting. The host
  also ships `newfs_udf` and `mount_udf` for applicable media and images.
  [M1, M6]
- The native NTFS access path is read-only. Paragon's NTFS for Mac and
  Tuxera's NTFS for Mac provide write access. The Paragon product also
  advertises formatting. Check the installed product's exact macOS release
  and license. Paragon's extFS for Mac offers ext4 read/write and format.
  Open-source FSKit options include read-only ExtendFS and beta Ext4Kit with
  writable mounts and a formatter. FSKit is an OS framework. The selected
  provider determines which on-disk format and operations it implements.
  [M2, M3, M5]
- NTFSTool's open-source edition offers writable NTFS mounts through FSKit.
  NTFS4Mac uses FSKit and libntfs-3g and adds formatting. Its published
  tests cover Intel Mac RAM disks, with Apple Silicon and Windows-formatted
  disks still pending in the project README. An NTFS-3G fork also targets
  FUSE-T. [M7]
- OpenZFS on macOS ships separately and has OS/architecture-specific builds.
  Pool compatibility with Linux and FreeBSD depends on the feature flags
  supported by the installed builds. [Z1, Z3]
- xlinuxfs uses LKL inside a signed FSKit extension for writable ext4,
  Btrfs, and XFS mounts. Its documented scope covers supported partitions
  and disk images. The OS release determines the raw-image attachment
  workflow. [M8]
- btrfs4mac documents a
  read-only FUSE mount and FSKit work. The `fuse-xfs` projects range from
  a historical read-only driver to a newer community fork that advertises
  optional writes. These are distinct paths from a native Linux mount.
  Check the supported XFS feature set and verify data preservation before
  enabling writes. [X6]

### FreeBSD

- UFS1 and UFS2 use `newfs`. ZFS uses the base `zpool` and `zfs` tools.
  FreeBSD's `ext2fs` can write compatible ext4. Treat missing Linux journal
  replay and other feature differences as a constraint for imported volumes.
  `e2fsprogs` supplies a formatter, which can enable features beyond the
  native mounting driver's support. [F1, U2, Z1]
- FreeBSD's exFAT and NTFS drivers use `fusefs`. The exFAT FUSE driver and
  format utilities are separate ports. NTFS-3G provides the writable NTFS
  route. Ports also include a read-only HFS+ driver and a HAMMER2 port with
  experimental writes. The separate `fusefs-lkl` package for Btrfs, ext4,
  and XFS is pinned to a Linux 4.16-era snapshot with an architecture
  restriction. Treat it as a
  compatibility path and check modern feature bits before accepting an image.
  [F1–F4, H1]

### OpenBSD and NetBSD

- OpenBSD FFS2 is the native general-purpose choice. Its native `ext2fs`
  permits a read-only subset of ext4. `exfat-fuse` and the `ntfs_3g`
  package provide separate FUSE routes with formatters. Native NTFS and UDF
  mounts are read-only. OpenBSD and NetBSD distribute `mkfs.ext4` in
  `e2fsprogs`. That formatter can produce feature profiles outside their
  native ext4 mount capability. [O1–O4, N2]
- NetBSD supports FFSv1/v2, LFS and ZFS, plus native writable UDF with media
  constraints. `fuse-exfat` and `fuse-ntfs-3g` provide writable access
  through the NetBSD userspace filesystem stack. The native NTFS driver
  allows only limited overwrites, and native NILFS2 and HFS+ are read-only.
  The native ext4 extent reader and the experimental writable `fuse-ext2`
  port have different support levels. [N1–N6, Z2]
- NetBSD provides metadata journaling for FFS through WAPBL. Compatibility
  with other FFS implementations depends in part on whether the volume was
  cleanly unmounted. [N7]

### DragonFly BSD

- UFS, HAMMER, and HAMMER2 are native and have separate formatters. HAMMER2
  is the maintained general-purpose target and DragonFly's default root
  filesystem. [D1]
- Stock **6.4.2** has `newfs_msdos`, `mount_msdos`, native
  `mount_ext2fs`, and legacy read-only NTFS/UDF. Its `ext2fs` source
  accepts only a subset of ext4 features. Native ext4 write compatibility
  for current Linux formatter defaults remains unverified. DPorts ships
  `e2fsprogs-core` for formatting. [D1, D4–D5]
- DPorts supplies `exfat-utils`, `fusefs-exfat`, `fusefs-ntfs`, and
  `fusefs-hfsfuse`. The base `mount_fusefs` helper exists in 6.4.2,
  while the release source excludes the `fuse` module from the default
  build. The development branch builds the module and has additional I/O
  fixes. Verify a working FUSE kernel and userland pair before exposing
  read/write exFAT or optional NTFS. Formatting alone does not establish
  mount support. [D2–D3]

### Windows

- NTFS is the general-purpose native choice. exFAT is native for portable
  volumes. FAT32 and UDF are compatibility choices. ReFS creation depends
  on Windows Server support and Windows 11 Dev Drive or edition rules.
  Microsoft documents different feature sets for NTFS and ReFS, so a
  supported ReFS volume does not provide every NTFS feature. [W1–W3]
- WinBtrfs provides an optional Windows mount driver and `mkbtrfs`.
  Check driver signing and the exact Windows build. Windows also has a
  release-candidate OpenZFS port. Paragon's Linux File Systems for Windows
  gives ext4 read/write and Btrfs read-only access. Its product page
  advertises XFS write access, while the vendor's product documentation
  describes XFS as read-only. Confirm the installed version before counting
  Windows as a writable XFS target. The APFS and HFS+ products use separate
  drivers, with APFS volume-type restrictions.
  [W5–W6, X2–X3, Z4]
- Janus implements experimental read-only ext4 through WinFsp. [W4]
- Ext4Fsd is a free Windows ext4 kernel driver. Upstream documents
  writable support for metadata checksums and 64-bit block numbers.
  Features such as `bigalloc` and `verity` force read-only access.
  Features such as `casefold` and `inline_data` prevent mounting.
  Its journal support is limited to internal journal replay. [W8]
- Ext4Windows uses WinFsp and lwext4 for writable Windows drive-letter
  mounts of raw ext4 images and partitions. Pengwin and
  `ext4-win-driver` provide separate WinFsp implementations with write
  paths documented upstream. Qualify container-backed virtual devices
  separately from physical partitions and raw images. [W9]

### Experimental ports and recovery tools

- OpenRefs has a userspace ReFS reader and FUSE frontend, plus an experimental
  Linux kernel driver. Its BSD and macOS routes depend on a matching FUSE
  environment. Record these as investigations, with read-only access and
  the exact ReFS version verified individually. Paragon also advertises a
  separate commercial Linux driver with write support for specified 1.x and
  3.x versions. Verify the exact build and volume profile. [X4–X5]
- The Linux APFS kernel project defaults to read-only and describes writes as
  experimental. `apfs-fuse` offers a separate read-only route. Paragon offers a
  commercial Linux read/write driver and a separate Windows driver with
  volume-type restrictions. [X1–X2]
- The open-source Windows `winapfs` beta reuses the Linux APFS driver in
  userspace over WinFsp. Its first public release describes writes as
  experimental and marks validation with Apple's `fsck_apfs` as pending.
  The separate `apfs_for_windows` project offers writable WinFsp mounts
  for selected unencrypted, unsealed volumes and documents directory size
  and depth limits for writes. [X8]
- HAMMER2 has separate FreeBSD, NetBSD, and OpenBSD work. The current FreeBSD
  port marks write support experimental. NetBSD and OpenBSD have development
  ports. Linux has an out-of-tree kernel project with formatting and write
  support under development, plus independent userspace image tools. [H1–H3]
- Commercial **UFS Explorer** on Windows, macOS, and Linux can inspect or
  recover files from APFS, HFS+, UFS, ZFS, XFS, Btrfs, ReFS, and other
  formats listed by the vendor. The experimental UFS2Tool has a Windows
  Dokan mount path for FreeBSD UFS images.
  [U3, S7]
- **FsRover** provides a free read-only mount as well as extraction on Windows
  through WinFsp or Dokany and on Linux through FUSE. Its documented formats
  include ext4, XFS, Btrfs, F2FS, APFS, UFS, JFS, ReiserFS, and ReFS. [X7]

### Network, virtual, and archive filesystems

SMB, NFS, WebDAV, SSHFS, CephFS, Lustre, BeeGFS, and other network or
distributed filesystems expose a tree from another machine or cluster.
OverlayFS, nullfs, tmpfs, procfs, and tarfs expose views of directories,
memory, kernel state, or archives. WIM, DMG, VHDX, GPT, and MBR are image or
partition formats. FUSE, FSKit, WinFsp, and Dokan are filesystem frameworks.
[L1, M5–M6, O4, W4, S2, S4–S5]

## Primary references

### Linux and shared formats

- **L1** Linux filesystem documentation and 7.2 source inventory. https://docs.kernel.org/filesystems/ · https://github.com/torvalds/linux/tree/v7.2/fs
- **L2** Linux 7.2 writable NTFS driver and formatter documentation. https://docs.kernel.org/7.2/filesystems/ntfs.html · https://github.com/ntfsprogs-plus/ntfsprogs-plus
- **L3** Linux NTFS3 driver. https://docs.kernel.org/7.2/filesystems/ntfs3.html
- **L4** Tuxera NTFS-3G source and utilities. https://github.com/tuxera/ntfs-3g
- **L5** Linux exFAT driver and utilities. https://docs.kernel.org/filesystems/exfat.html · https://github.com/exfatprogs/exfatprogs
- **L6** bcachefs upstream release and DKMS status. https://bcachefs.org/ · https://github.com/koverstreet/bcachefs-tools/blob/master/Changelog.mdwn
- **L7** Filesystem tools and F2FS format documentation. https://git.kernel.org/pub/scm/fs/ext2/e2fsprogs.git/ · https://github.com/kdave/btrfs-progs · https://git.kernel.org/pub/scm/fs/xfs/xfsprogs-dev.git/ · https://docs.kernel.org/filesystems/f2fs.html · https://github.com/dosfstools/dosfstools
- **L8** Linux compatibility filesystem modes and formatter source. https://docs.kernel.org/filesystems/adfs.html · https://docs.kernel.org/filesystems/affs.html · https://docs.kernel.org/filesystems/befs.html · https://docs.kernel.org/filesystems/hpfs.html · https://github.com/torvalds/linux/blob/v7.2/fs/qnx4/Kconfig · https://github.com/torvalds/linux/blob/v7.2/fs/qnx6/Kconfig · https://docs.kernel.org/filesystems/omfs.html · https://docs.kernel.org/filesystems/romfs.html · https://github.com/util-linux/util-linux/blob/master/disk-utils/mkfs.minix.c
- **L9** Linux zonefs and zoned-device constraints. https://docs.kernel.org/filesystems/zonefs.html

### macOS and Windows

- **M1** Apple Disk Utility filesystem formats. https://support.apple.com/guide/disk-utility/file-system-formats-dsku19ed921c/22.7/mac/27
- **M2** Paragon extFS for Mac. https://www.paragon-software.com/home/extfs-mac/
- **M3** Paragon and Tuxera NTFS for Mac. https://www.paragon-software.com/home/ntfs-mac/ · https://ntfsformac.tuxera.com/
- **M5** Apple FSKit and ext4 provider examples. https://developer.apple.com/documentation/FSKit · https://github.com/kthchew/ExtendFS · https://github.com/rayhanadev/Ext4Kit
- **M6** Apple filesystem details, including UDF and optical filesystems. https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemDetails/FileSystemDetails.html
- **M7** Open-source macOS NTFS drivers through FSKit and FUSE-T. https://github.com/ntfstool/ntfstool · https://github.com/vahidzekic/ntfs4mac · https://github.com/macos-fuse-t/ntfs-3g
- **M8** macOS ext4, Btrfs, and XFS via LKL and FSKit. https://github.com/HuanchuanTech/xlinuxfs
- **W1** Microsoft Windows format command. https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/format
- **W2** Microsoft filesystem feature comparison and exFAT specification. https://learn.microsoft.com/en-us/windows/win32/fileio/filesystem-functionality-comparison · https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification
- **W3** Microsoft ReFS and Windows 11 Dev Drive. https://learn.microsoft.com/en-us/windows-server/storage/refs/refs-overview · https://learn.microsoft.com/en-us/windows/dev-drive/
- **W4** Janus read-only driver and Windows filesystem frameworks. https://github.com/SuperCoolPencil/janus · https://github.com/winfsp/winfsp · https://github.com/dokan-dev/dokany
- **W5** Paragon Linux File Systems for Windows product page and product brief. https://www.paragon-software.com/home/linuxfs-windows/ · https://dl.paragon-software.com/doc/Linux_File_Systems_for_Windows_OP.pdf
- **W6** WinBtrfs driver and formatter. https://github.com/maharmstone/btrfs
- **W7** Microsoft in-box filesystem driver overview. https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/about-file-system-drivers
- **W8** Ext4Fsd writable features and signed Windows release. https://github.com/bobranten/Ext4Fsd · https://github.com/bobranten/Ext4Fsd/releases/tag/v0.71
- **W9** WinFsp ext4 write paths. https://github.com/Mateuscruz19/Ext4Windows · https://github.com/althk/pengwin · https://github.com/antimatter-studios/ext4-win-driver

### BSDs and sharing families

- **F1** FreeBSD filesystem handbook and release baseline. https://docs.freebsd.org/en/books/handbook/filesystems/ · https://www.freebsd.org/releases/15.1R/relnotes/
- **F2** FreeBSD exFAT driver and format ports. https://cgit.freebsd.org/ports/tree/filesystems/exfat/Makefile · https://cgit.freebsd.org/ports/tree/filesystems/exfat-utils/Makefile
- **F3** FreeBSD NTFS and HFS+ packages. https://cgit.freebsd.org/ports/tree/filesystems/ntfs/Makefile · https://github.com/0x09/hfsfuse
- **F4** FreeBSD LKL and XFS formatter ports. https://raw.githubusercontent.com/freebsd/freebsd-ports/main/filesystems/lkl/Makefile · https://cgit.freebsd.org/ports/tree/filesystems/xfsprogs
- **F5** FreeBSD UDF source and tools, tarfs and image tooling. https://github.com/freebsd/freebsd-src/blob/releng/15.1/sys/fs/udf/udf_vfsops.c · https://cgit.freebsd.org/ports/tree/sysutils/udfclient/Makefile?id=e956200e0cb1ab3a412c123fa23aaaf47b1ea8da · https://man.freebsd.org/cgi/man.cgi?query=tarfs&sektion=5 · https://man.freebsd.org/cgi/man.cgi?query=makefs&sektion=8
- **O1** OpenBSD 8.0, disk setup and filesystem types. https://www.openbsd.org/80.html · https://www.openbsd.org/faq/faq14.html · https://man.openbsd.org/fstab · https://man.openbsd.org/newfs
- **O2** OpenBSD ext2/ext4 mount implementation and package formatter. https://man.openbsd.org/mount_ext2fs · https://github.com/openbsd/src/blob/master/sys/ufs/ext2fs/ext2fs_vfsops.c · https://raw.githubusercontent.com/openbsd/ports/master/sysutils/e2fsprogs/pkg/PLIST
- **O3** OpenBSD base NTFS, optional exFAT, and writable NTFS package. https://man.openbsd.org/mount_ntfs · https://raw.githubusercontent.com/openbsd/ports/master/sysutils/exfat-fuse/pkg/PLIST · https://raw.githubusercontent.com/openbsd/ports/master/sysutils/ntfs-3g/Makefile · https://man.openbsd.org/fuse
- **O4** OpenBSD UDF, general mounts, and image creation. https://github.com/openbsd/src/blob/master/sys/isofs/udf/udf_vfsops.c · https://man.openbsd.org/mount · https://man.openbsd.org/makefs
- **N1** NetBSD 11.1, native formats, and interoperability. https://www.netbsd.org/releases/formal-11/NetBSD-11.1.html · https://www.netbsd.org/about/interop.html · https://man.netbsd.org/NetBSD-11.1/newfs.8 · https://man.netbsd.org/mount_hfs.8
- **N2** NetBSD ext4 extent implementation, formatter, and FUSE writer limitations. https://www4.netbsd.org/gallery/presentations/hrishikesh/2017_AsiaBSDCon/abc2017ext4_final_paper.pdf · https://github.com/NetBSD/src/blob/netbsd-11/sys/ufs/ext2fs/ext2fs_vfsops.c · https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/sysutils/e2fsprogs/PLIST · https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/filesystems/fuse-ext2/DESCR
- **N3** NetBSD exFAT and NTFS packages and base NTFS modes. https://wwwcdn.netbsd.org/pub/pkgsrc/current/pkgsrc/filesystems/fuse-exfat/index.html · https://wwwcdn.netbsd.org/pub/pkgsrc/current/pkgsrc/filesystems/fuse-ntfs-3g/index.html · https://man.netbsd.org/mount_ntfs.8
- **N4** NetBSD LFS and NILFS2. https://man.netbsd.org/newfs_lfs.8 · https://github.com/NetBSD/src/blob/netbsd-11/sbin/mount_nilfs/mount_nilfs.8
- **N5** NetBSD UDF and image tools. https://man.netbsd.org/newfs_udf.8 · https://man.netbsd.org/mount_udf.8 · https://man.netbsd.org/makefs.8
- **N6** NetBSD specialty filesystems. https://man.netbsd.org/NetBSD-11.x-BRANCH/mount_chfs.8 · https://man.netbsd.org/mount_filecore.8 · https://man.netbsd.org/newfs_v7fs.8 · https://man.netbsd.org/newfs_sysvbfs.8
- **N7** NetBSD WAPBL support and interoperability limits. https://man.netbsd.org/NetBSD-11.0/wapbl.4
- **D1** DragonFly release, native filesystems, and tooling. https://www.dragonflybsd.org/release64/ · https://www.dragonflybsd.org/docs/handbook/UnixBasics/ · https://man.dragonflybsd.org/?command=newfs_hammer2&section=8
- **D2** DragonFly 6.4.2 FUSE exclusion and development branch changes. https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/Makefile · https://github.com/DragonFlyBSD/DragonFlyBSD/commit/4d178a6a8bdff7603d9bcb6c5fff68a4e2efd337 · https://github.com/DragonFlyBSD/DragonFlyBSD/commit/053116cafd76a7ce620ebdfedcf6869e986e327d
- **D3** DragonFly optional packages and exFAT driver upstream. https://github.com/DragonFlyBSD/DPorts · https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/filesystems/exfat-utils/Makefile · https://github.com/relan/exfat
- **D4** DragonFly ext-family driver and formatter. https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ext2fs/ext2fs.h · https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ext2fs/ext2_vfsops.c · https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/filesystems/e2fsprogs-core/Makefile
- **D5** DragonFly compatibility mounts. https://man.dragonflybsd.org/?command=mount_ntfs&section=8 · https://man.dragonflybsd.org/?command=mount_udf&section=8 · https://man.dragonflybsd.org/?command=mount_hpfs&section=8
- **U1** Linux UFS variants and experimental write configuration. https://docs.kernel.org/admin-guide/ufs.html · https://github.com/torvalds/linux/blob/v7.2/fs/ufs/Kconfig
- **U2** FreeBSD UFS formatter and the DragonFly UFS definition. https://man.freebsd.org/cgi/man.cgi?query=newfs&sektion=8 · https://www.dragonflybsd.org/docs/handbook/Installation/ · https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ufs/fs.h
- **U3** FreeBSD UFS images with Windows Dokan mounts. https://github.com/SvenGDK/UFS2Tool

### Optional, enterprise, and recovery paths

- **Z1** OpenZFS platform support and pool feature flags. https://github.com/openzfs/zfs · https://openzfs.github.io/openzfs-docs/man/master/7/zpool-features.7.html
- **Z2** NetBSD ZFS port and module. https://wiki.netbsd.org/zfs/ · https://github.com/NetBSD/src/blob/netbsd-11/sys/modules/zfs/Makefile
- **Z3** OpenZFS on macOS releases. https://github.com/openzfsonosx/openzfs-fork/releases
- **Z4** OpenZFS on Windows release candidates. https://github.com/openzfsonwindows/openzfs/releases
- **H1** HAMMER2 FreeBSD port and packaging. https://github.com/kusumi/freebsd_hammer2 · https://raw.githubusercontent.com/freebsd/freebsd-ports/main/filesystems/hammer2/Makefile
- **H2** HAMMER2 NetBSD and OpenBSD ports. https://github.com/kusumi/netbsd_hammer2 · https://github.com/kusumi/openbsd_hammer2
- **H3** HAMMER2 portable image tools and Linux research. https://github.com/kusumi/hammer2 · https://github.com/kusumi/hammer2-utils · https://github.com/jdmanring/linux_hammer2
- **X1** APFS open-source readers, experimental writer, and FreeBSD port work. https://github.com/sgan81/apfs-fuse · https://github.com/linux-apfs/linux-apfs-rw · https://github.com/sgan81/apfs-fuse/pull/139
- **X2** Paragon APFS for Linux and Windows. https://www.paragon-software.com/home/apfs-linux/ · https://www.paragon-software.com/us/home/apfs-windows/
- **X3** Paragon HFS+ for Windows and Linux, and HFS+ FUSE reader. https://www.paragon-software.com/home/hfs-windows/ · https://www.paragon-software.com/us/about/products-a-to-z/ · https://github.com/0x09/hfsfuse
- **X4** OpenRefs experimental kernel driver and userspace frontend. https://github.com/unsound/openrefs
- **X5** Paragon commercial ReFS read/write driver for Linux. https://www.paragon-software.com/us/business/refs-linux/ · https://www.paragon-drivers.com/en/refslin/
- **X6** macOS community Btrfs and XFS drivers. https://github.com/gereoffy/btrfs4mac · https://github.com/karan-vk/fuse-xfs · https://sourceforge.net/p/fusexfs/wiki/Home/
- **X7** FsRover format and read-only mount support. https://a1ive.github.io/FsRover/ · https://github.com/a1ive/FsRover
- **X8** Windows open-source APFS write paths and release limits. https://github.com/christofilojohn/winapfs/releases · https://github.com/RandyNorthrup/apfs_for_windows#supported-scope
- **S1** EROFS and SquashFS projects, plus current NetBSD tools catalog. https://erofs.docs.kernel.org/en/latest/ · https://github.com/plougher/squashfs-tools · https://wwwcdn.netbsd.org/pub/pkgsrc/current/pkgsrc/filesystems/index.html
- **S2** Linux shared disk and cluster documentation. https://docs.kernel.org/filesystems/gfs2/index.html · https://docs.kernel.org/filesystems/ocfs2.html
- **S3** Veritas InfoScale and VxFS support. https://www.veritas.com/support/en_US/doc/infoscale_scl_91_lin · https://www.veritas.com/content/support/en_US/doc/79722584-170945085-0/v41267960-170945085
- **S4** Xsan and StorNext interoperability. https://support.apple.com/en-ie/102855 · https://www.quantum.com/en/products/file-system/stornext/
- **S5** IBM Storage Scale software requirements. https://www.ibm.com/docs/en/storage-scale/6.0.1?topic=gpfs-software-requirements
- **S6** FreeBSD compatibility tools and historical ReiserFS source. https://cgit.freebsd.org/ports/tree/filesystems/hfsutils · https://cgit.freebsd.org/ports/tree/filesystems/progsreiserfs · https://github.com/torvalds/linux/tree/v6.12/fs/reiserfs
- **S7** UFS Explorer recovery format support. https://www.ufsexplorer.com/ufs-explorer-professional-recovery/ · https://www.ufsexplorer.com/kb-technologies/
