# Filesystem implementation assessment

Assessed 2026-11-24 against the benchmark in
[the tag guide](filesystem-research-tags.md#implementation-completeness).
Covers all 160 restored matrix offerings. Identical findings for the same
implementation on multiple OSes share a row; OS-specific behavior stays separate.
The [matrix](filesystem-support-matrix.md) keeps only compact tags.

For the separate 2026-11-26 review of Lurker's integration code, see the
[filesystem integration complexity audit](filesystem-integration-complexity-audit.md).
It distinguishes required adaptations, provider defects and potential
simplifications; it does not change this capability survey or the promise table.

- **Partial**: a documented important feature gap; prefix the matrix entry with `Partial`.
- **Largely implemented**: no important gap identified for the stated operation;
  minor or optional exceptions remain in the notes. No completeness prefix.
- **Coverage unresolved** (`Partial*`): sources confirm the advertised operation
  but do not establish enough feature detail to judge completeness. Prefix the
  matrix entry with `Partial*`; each finding below records the unresolved
  features/evidence and the sources checked. Unknown completeness must not
  silently mean fully/largely implemented.

All status tags precede capabilities: `[Paid] [Beta] [Partial or Partial*]
[Outdated] Format Mount Access [(product number)]`. These four dimensions are
independent; their free, production, fully/largely implemented and current
values are implicit. Use `Partial` for confirmed important gaps even if other
features remain uncertain; never combine it with `Partial*`.

These are evidence-based research judgments, not independent runtime tests or
crash-safety certification. Read-only implementations are assessed on reading
and metadata exposure; writable ones also on mutation and recovery. Formatters
are assessed on creation; lack of a mount is already NM. An OS restriction or
missing module is recorded separately from an implementation defect.

## Later macOS planning updates

The 2026-11-25 native CLI follow-up raises two planning promises beyond the
original research snapshot below:

- **Btrfs: ☑️¹ → ☑️.** The approved anylinuxfs Linux guest formats and mounts
  the decrypted virtual disk; authenticated SFTP/macFUSE SSHFS exposes it to
  macOS applications. Actual VeraCrypt container creation and read/write
  persistence passed. The original anylinuxfs/NFS assessment below remains
  specific to that earlier interface, not the qualified SSHFS combination.
- **UFS2 / FFSv2: ⚠️ R/O → ☑️².** Patched UFS2Tool formatting plus patched
  fuse-ufs/macFUSE passed actual VeraCrypt Create + Read/Write persistence.
  This is a combined formatter and mount driver, not UFS2Tool's Windows-only
  mount command. Cross-parent directory moves are refused; the writer remains
  experimental and does not provide full UFS2 feature or reliability parity.

Both used normal close/reopen, hash checks, append/add-file and final reopen
verification. No exhaustive feature audit or physical-device, desktop or
release-package qualification is implied. The
[updated planning notes](filesystem-support-matrix.md#macos-promise-updates),
[native receipts](../DIRECTIVES/PLAN/FILESYSTEM_ROUND/MACOS_RETEST.md),
[setup documentation](macos-filesystems.md) and
[UFS2 patches and source pins](../native/macos/fuse-ufs.md) establish the
new combinations. The original 160 per-solution findings remain a dated survey;
these two updates do not reclassify its other offerings.

## Benchmark references

The tag guide lists the common operations and filesystem-specific features.
Reference implementations are Linux ext4/Btrfs/XFS, Apple APFS, each BSD’s own
UFS/FFS variant, DragonFly HAMMER2, Windows NTFS/exFAT and current OpenZFS.
Special boot/system-volume roles, vendor recovery utilities and universal
cross-version compatibility are not implied. Built-in, FUSE, FSKit, NFS and
virtual-machine transports are judged by the features ordinary apps receive.
Optional EFS decryption alone does not trigger Partial; losing NTFS streams,
normal metadata, compression operations or transactional recovery can. Likewise,
features absent from exFAT’s design do not count as implementation defects.

## Per-solution findings

### ext4

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | ext4 + e2fsprogs | **Largely implemented** — Reference Linux implementation and standard formatter; no important implementation gap identified for ordinary data-volume use. | E1 |
| Linux; Windows | FsRover | **Partial** — Does not replay the journal; encrypted file contents are ciphertext. | E4 |
| macOS | xlinuxfs | **Partial** — Linux filesystem engine, but mounted access omits POSIX ACL mapping, UID/GID changes and non-user xattr namespaces. | E5 |
| macOS | anylinuxfs | **Coverage unresolved** (`Partial*`) — Uses Linux drivers and OpenZFS through NFS. Broad file access is documented; preservation of each filesystem’s ACLs, xattrs and special operations through NFS is not established. | E6 |
| macOS | PolyDrive Disk | **Coverage unresolved** (`Partial*`) — Vendor advertises normal file operations and journal recovery; published material does not establish complete metadata and filesystem-specific feature coverage. | E8 |
| macOS | ExtendFS | **Partial** — Needs-recovery volumes are not repaired, checksum verification is incomplete, and Finder omits duplicate same-directory hardlinks. Refuses unsupported incompatible flags. | E9 |
| macOS | Ext4Kit | **Partial** — No sparse-file allocation; holes are zero-filled. Also rejects inline_data volumes and restricts xattr values to one filesystem block. | E10 |
| macOS | open_ext4_for_mac | **Partial** — Rejects several ext4 profiles including casefold, large_dir and ea_inode; quota/project/verity profiles are read-only. Journal support does not remove those restrictions. | E11 |
| macOS | Paragon extFS for Mac | **Partial** — Current product page excludes metadata_csum, a mainstream ext4 feature, plus bigalloc, external journal, meta_bg and inline_data. Paid status does not imply complete ext4 support. | E12 |
| macOS | vmount Pro | **Coverage unresolved** (`Partial*`) — Formatting and ordinary mounts are advertised. The public product description does not establish the metadata, recovery and format-feature benchmark for each filesystem. | E13 |
| FreeBSD | ext2fs + e2fsprogs | **Partial** — No journaling or encryption; extended attributes are experimental. An independent full formatter does not fix the mount driver’s implementation gaps. | E14 |
| FreeBSD | fusefs-ext2 | **Partial** — No mounted set/list/remove-xattr callbacks; getxattr handles only user attributes. Important metadata/ACL coverage is missing independently of the upstream write-safety warning. | E15 |
| FreeBSD | fusefs-lkl | **Partial** — Pinned Linux 4.16-era engine cannot handle later incompatible features such as orphan_file. This is inferred from the pinned kernel’s feature set, independently of its age. | E16 |
| OpenBSD | ext2fs + e2fsprogs | **Partial** — Accepts only a subset of ext4 incompatible flags; mainstream 64-bit ext4 profiles are outside its supported mask. | E14 |
| NetBSD | ext2fs + e2fsprogs | **Partial** — Supports extents/64-bit layouts but rejects csum_seed and needs-recovery incompatible flags; normal modern profiles or an unreplayed journal can prevent mounting. | E14 |
| NetBSD | fuse-ext2 | **Partial** — No mounted set/list/remove-xattr callbacks; getxattr handles only user attributes. Important metadata/ACL coverage is missing independently of the upstream write-safety warning. | E15 |
| DragonFly BSD | ext2fs + e2fsprogs-core | **Partial** — DragonFly 6.4.2 omits needs-recovery from its accepted incompatible flags, so a normal ext4 journal requiring replay prevents mounting. The formatter does not broaden this reader. | E14 |
| Windows | DiskInternals Ext4 Formatter | **Coverage unresolved** (`Partial*`) — Creates ext4 according to vendor documentation. The exact generated feature profile and formatter validation coverage are not published; NM already records no mount in this offering. | E17 |
| Windows | DiskGenius Free | **Coverage unresolved** (`Partial*`) — Creates ext4 according to vendor documentation. The exact generated feature profile and formatter validation coverage are not published; NM already records no mount in this offering. | E17 |
| Windows | MiniTool Partition Wizard Free | **Coverage unresolved** (`Partial*`) — Creates ext4 according to vendor documentation. The exact generated feature profile and formatter validation coverage are not published; NM already records no mount in this offering. | E17 |
| Windows | Ext4Fsd | **Partial** — Documents internal-journal replay rather than a full write-journaling implementation; rejects casefold/large-directory/inline-data profiles and downgrades several other profiles to read-only. | E18 |
| Windows | CrossDriver Free | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | Paragon Linux File Systems for Windows | **Coverage unresolved** (`Partial*`) — Current global page documents ext4/XFS writes and Btrfs reads; older/regional pages differ. Exact installed version matters, and a complete feature matrix is not published. | E20 |
| Windows | DiskInternals Linux Writer Pro | **Coverage unresolved** (`Partial*`) — Drive-letter access is advertised. Complete metadata semantics and compatible feature profiles are not documented; file preview/export alone is excluded from the research bar. | E21 |
| Windows | CrossDriver for Linux Disks Pro | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | DiskInternals Linux Reader Pro | **Coverage unresolved** (`Partial*`) — Drive-letter access is advertised. Complete metadata semantics and compatible feature profiles are not documented; file preview/export alone is excluded from the research bar. | E21 |

### Btrfs

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | Btrfs + btrfs-progs | **Largely implemented** — Reference Linux implementation and standard formatter; no important implementation gap identified for ordinary data-volume use. | E1 |
| Linux; Windows | FsRover | **Partial** — Mount bridge lacks ACL/xattr operations and substitutes host ownership on Linux; compressed data and many RAID profiles remain readable. | E4 |
| macOS | xlinuxfs | **Partial** — Linux filesystem engine, but mounted access omits POSIX ACL mapping, UID/GID changes and non-user xattr namespaces. | E5 |
| macOS | anylinuxfs | **Coverage unresolved** (`Partial*`) — Uses Linux drivers and OpenZFS through NFS. Broad file access is documented; preservation of each filesystem’s ACLs, xattrs and special operations through NFS is not established. | E6 |
| macOS | PolyDrive Disk | **Coverage unresolved** (`Partial*`) — Vendor advertises normal file operations and journal recovery; published material does not establish complete metadata and filesystem-specific feature coverage. | E8 |
| macOS | btrfs4mac | **Partial** — Single-device volumes only and no log-tree replay; multi-device/RAID layouts are not supported. These are additional restrictions beyond read-only access. | E22 |
| macOS | vmount Pro | **Coverage unresolved** (`Partial*`) — Formatting and ordinary mounts are advertised. The public product description does not establish the metadata, recovery and format-feature benchmark for each filesystem. | E13 |
| FreeBSD | fusefs-lkl | **Partial** — Pinned Linux 4.16-era engine lacks later RAID1C3/RAID1C4 profiles and newer checksum choices. This is inferred from the pinned kernel’s feature set, independently of its age. | E16 |
| Windows | WinBtrfs | **Partial** — Broad compression, snapshots, checksums and RAID coverage, but full transaction-log support remains on the upstream TODO list. | E23 |
| Windows | CrossDriver Free | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | Paragon Linux File Systems for Windows | **Coverage unresolved** (`Partial*`) — Current global page documents ext4/XFS writes and Btrfs reads; older/regional pages differ. Exact installed version matters, and a complete feature matrix is not published. | E20 |
| Windows | CrossDriver for Linux Disks Pro | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | DiskInternals Linux Reader Pro | **Coverage unresolved** (`Partial*`) — Drive-letter access is advertised. Complete metadata semantics and compatible feature profiles are not documented; file preview/export alone is excluded from the research bar. | E21 |

### XFS

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | XFS + xfsprogs | **Largely implemented** — Reference Linux implementation and standard formatter; no important implementation gap identified for ordinary data-volume use. | E1 |
| Linux; Windows | FsRover | **Partial** — Mount bridge lacks ACL/xattr operations and substitutes host ownership on Linux; reading XFS v4/v5 data does not preserve native metadata semantics. | E4 |
| macOS | xlinuxfs | **Partial** — Linux filesystem engine, but mounted access omits POSIX ACL mapping, UID/GID changes and non-user xattr namespaces. | E5 |
| macOS | anylinuxfs | **Coverage unresolved** (`Partial*`) — Uses Linux drivers and OpenZFS through NFS. Broad file access is documented; preservation of each filesystem’s ACLs, xattrs and special operations through NFS is not established. | E6 |
| macOS | PolyDrive Disk | **Coverage unresolved** (`Partial*`) — Vendor advertises normal file operations and journal recovery; published material does not establish complete metadata and filesystem-specific feature coverage. | E8 |
| macOS | fuse-xfs | **Partial** — No reflinks, ACLs, xattr writes or quotas; external-log and realtime layouts are excluded. | E24 |
| macOS | vmount Pro | **Coverage unresolved** (`Partial*`) — Formatting and ordinary mounts are advertised. The public product description does not establish the metadata, recovery and format-feature benchmark for each filesystem. | E13 |
| FreeBSD | xfsprogs | **Largely implemented** — Ported upstream formatter; no important format-operation gap identified. This entry makes no claim of an XFS mount. | E25 |
| FreeBSD; OpenBSD | libfsxfs | **Partial** — Rejects quota-enabled XFS and meta_uuid, among other incompatible features. Ordinary read-only mode does not describe these layout restrictions. | E26 |
| FreeBSD | fusefs-lkl | **Partial** — Pinned Linux 4.16-era engine predates bigtime and inobtcount, used by modern formatters. This is inferred from the pinned kernel’s feature set, independently of its age. | E16 |
| Windows | CrossDriver Free | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | Paragon Linux File Systems for Windows | **Coverage unresolved** (`Partial*`) — Current global page documents ext4/XFS writes and Btrfs reads; older/regional pages differ. Exact installed version matters, and a complete feature matrix is not published. | E20 |
| Windows | CrossDriver for Linux Disks Pro | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | DiskInternals Linux Reader Pro | **Coverage unresolved** (`Partial*`) — Drive-letter access is advertised. Complete metadata semantics and compatible feature profiles are not documented; file preview/export alone is excluded from the research bar. | E21 |

### APFS

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | apfs-fuse | **Partial** — No transparent LZFSE decompression. Some compressed file content is therefore unreadable as normal file data, a feature gap beyond read-only mode. | E27 |
| Linux | linux-apfs-rw + apfsprogs | **Partial** — No encrypted-volume support, including reads; Fusion writes are unavailable. Experimental writes cover a restricted APFS profile. | E28 |
| Linux; FreeBSD; OpenBSD | libfsapfs | **Partial** — No snapshot or Fusion support and incomplete compression-method coverage. Encryption support does not remove those gaps. | E29 |
| Linux; Windows | FsRover | **Partial** — Skips FileVault volumes and does not support snapshots or Fusion containers. | E4 |
| Linux | Paragon APFS for Linux | **Partial** — Encrypted/snapshot volumes are read-only; Fusion and T2-encrypted storage are excluded. Native APFS encrypted-volume writing is an important missing capability. | E30 |
| macOS | Disk Utility / APFS | **Largely implemented** — Reference macOS implementation; device and image workflows are documented. | E2 |
| Windows | CrossDriver for Mac Disks Free | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| Windows | winapfs | **Partial** — Writes exclude encrypted volumes, snapshots and Fusion layouts; readable encrypted volumes use a separate read-only path. | E31 |
| Windows | apfs_for_windows | **Partial** — Writable volumes are limited by directory entry count and nesting depth; new hardlinks and encrypted/sealed volumes are unsupported. | E32 |
| Windows | Paragon APFS for Windows | **Partial** — Some compressed files are unreadable; encrypted and snapshot volumes are read-only, and writable support requires a compatible single-store volume. | E33 |
| Windows | OWC MacDrive 12 | **Coverage unresolved** (`Partial*`) — Vendor documents APFS reads/writes and encrypted-volume support, but not enough detail to close every benchmark feature. APFS formatting remains unverified in the original survey. | E34 |
| Windows | DiskInternals Linux Reader Pro | **Coverage unresolved** (`Partial*`) — Drive-letter access is advertised. Complete metadata semantics and compatible feature profiles are not documented; file preview/export alone is excluded from the research bar. | E21 |
| Windows | CrossDriver for Mac Disks Pro | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |

### UFS1 / FFSv1

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | makefs + Linux UFS | **Partial** — UFS inode-operation tables lack xattr and ACL handlers; selecting the correct BSD variant remains necessary. Experimental write mode is a separate issue. | E35 |
| Linux | Linux UFS write option + makefs | **Partial** — UFS inode-operation tables lack xattr and ACL handlers; selecting the correct BSD variant remains necessary. Experimental write mode is a separate issue. | E35 |
| Linux; macOS | UFS2Tool | **Coverage unresolved** (`Partial*`) — Archived formatter creates UFS images; exact supported output profiles are not fully documented. Offline extraction/editing is outside the bar and is not counted as a mount. | E36 |
| Linux; Windows | FsRover | **Partial** — Mount bridge lacks ACL/xattr operations and substitutes host ownership on Linux; BSD/Solaris file reading is not complete variant-specific metadata access. | E4 |
| FreeBSD | UFS1 | **Largely implemented** — Native FreeBSD implementation is the reference for its own UFS variant; cross-BSD incompatibility is not itself a missing implementation feature. | E37 |
| OpenBSD | FFSv1 | **Largely implemented** — Native implementation is the reference for this OS’s FFS variant. NetBSD FFSv2ea and cross-BSD variant compatibility remain separate qualifications. | E38 |
| NetBSD | FFSv1 | **Largely implemented** — Native implementation is the reference for this OS’s FFS variant. NetBSD FFSv2ea and cross-BSD variant compatibility remain separate qualifications. | E39 |
| DragonFly BSD | DragonFly UFS | **Largely implemented** — Native DragonFly implementation is the reference for its own format. UFS1 is not interchangeable with another BSD’s UFS2. | E40 |
| Windows | UFS2Tool | **Partial** — Dokan mount rejects new-file creation; modifying existing image files does not implement normal writable UFS usage. | E36 |

### UFS2 / FFSv2

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | makefs + Linux UFS | **Partial** — UFS inode-operation tables lack xattr and ACL handlers; selecting the correct BSD variant remains necessary. Experimental write mode is a separate issue. | E35 |
| Linux | Linux UFS write option + makefs | **Partial** — UFS inode-operation tables lack xattr and ACL handlers; selecting the correct BSD variant remains necessary. Experimental write mode is a separate issue. | E35 |
| Linux; macOS | UFS2Tool | **Coverage unresolved** (`Partial*`) — Archived formatter creates UFS images; exact supported output profiles are not fully documented. Offline extraction/editing is outside the bar and is not counted as a mount. | E36 |
| Linux; FreeBSD; OpenBSD | fuse-ufs | **Partial** — FreeBSD UFS2 implementation explicitly lacks ACL support; experimental writes do not imply complete FreeBSD UFS2 semantics. | E41 |
| Linux; Windows | FsRover | **Partial** — Mount bridge lacks ACL/xattr operations and substitutes host ownership on Linux; BSD/Solaris file reading is not complete variant-specific metadata access. | E4 |
| FreeBSD | UFS2 | **Largely implemented** — Native FreeBSD implementation is the reference for its own UFS variant; cross-BSD incompatibility is not itself a missing implementation feature. | E37 |
| OpenBSD | FFSv2 | **Largely implemented** — Native implementation is the reference for this OS’s FFS variant. NetBSD FFSv2ea and cross-BSD variant compatibility remain separate qualifications. | E38 |
| NetBSD | FFSv2 | **Largely implemented** — Native implementation is the reference for this OS’s FFS variant. NetBSD FFSv2ea and cross-BSD variant compatibility remain separate qualifications. | E39 |
| DragonFly BSD | portable makefs | **Largely implemented** — Generates filesystem images using the ported formatter. No important formatting-only gap identified; NM already records the absence of a mount. | E42 |
| Windows | UFS2Tool | **Partial** — Dokan mount rejects new-file creation; modifying existing image files does not implement normal writable UFS usage. | E36 |

### HAMMER2

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | linux_hammer2 + hammer2-utils | **Largely implemented** — Documents broad local-volume operations, compression, checksums, deduplication and snapshots. Remote replication is limited; experimental clustering is not the ordinary local-filesystem benchmark. | E43 |
| Linux; FreeBSD | hammer2-fuse | **Coverage unresolved** (`Partial*`) — Read-only frontend reuses HAMMER2 code; exact mounted metadata and layout coverage is not comprehensively documented. Read-only mode alone does not justify Partial. | E44 |
| Linux; FreeBSD; OpenBSD; NetBSD | portable makefs | **Largely implemented** — Generates filesystem images using the ported formatter. No important formatting-only gap identified; NM already records the absence of a mount. | E42 |
| FreeBSD | hammer2 port | **Coverage unresolved** (`Partial*`) — Experimental port of DragonFly HAMMER2. Complete parity of mounted metadata, recovery and multi-volume behavior is not established by the public port description. | E45 |
| DragonFly BSD | HAMMER2 | **Largely implemented** — Reference HAMMER2 implementation for local data volumes, including checksums, compression and PFS/snapshots. | E40 |
| Windows | portable makefs (Cygwin) | **Largely implemented** — Generates filesystem images using the ported formatter. No important formatting-only gap identified; NM already records the absence of a mount. | E42 |

### NTFS

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | kernel NTFS + mkntfs | **Partial** — Kernel choice matters: ntfs3 lists full write journaling as unfinished; the newer ntfs driver rejects explicit size changes for compressed files. This is not one uniform kernel implementation. | E46 |
| Linux; FreeBSD | NTFS-3G | **Partial** — Normal file/metadata support is broad, but writes are not logged as Windows NTFS transactions; default unclean-volume recovery clears the Windows log. | E47 |
| Linux; Windows | FsRover | **Partial** — No alternate data streams; encrypted file contents are ciphertext, and reparse handling covers only selected types. | E4 |
| Linux | Paragon NTFS for Linux Free | **Coverage unresolved** (`Partial*`) — Vendor documents broad NTFS support. A current exhaustive feature/recovery comparison is unavailable; Free lacks the Professional formatter. Do not substitute ntfs3’s limitations for this commercial engine. | E48 |
| Linux | Paragon NTFS for Linux Professional | **Coverage unresolved** (`Partial*`) — Vendor documents broad NTFS support. A current exhaustive feature/recovery comparison is unavailable; Free lacks the Professional formatter. Do not substitute ntfs3’s limitations for this commercial engine. | E48 |
| macOS | macOS | **Coverage unresolved** (`Partial*`) — Built-in reads are documented. Current feature-by-feature mounted coverage, including Windows reparse-point behavior, is not established here. Read-only alone is not Partial. | E49 |
| macOS | PolyDrive Disk | **Coverage unresolved** (`Partial*`) — Vendor advertises normal file operations and journal recovery; published material does not establish complete metadata and filesystem-specific feature coverage. | E8 |
| macOS | NTFSTool | **Coverage unresolved** (`Partial*`) — Named-stream xattrs are documented, but the NTFS engine is supplied as a prebuilt library. Open app/glue source does not establish full engine coverage. | E50 |
| macOS | TT NTFS Native | **Partial** — Upstream lists creating compressed files as not yet supported. Documented Windows-compatible journal repair does not establish every NTFS feature. | E51 |
| macOS | NTFS-3G + macFUSE | **Partial** — Normal file/metadata support is broad, but writes are not logged as Windows NTFS transactions; default unclean-volume recovery clears the Windows log. | E47 |
| macOS | NTFS-3G for FUSE-T | **Partial** — Normal file/metadata support is broad, but writes are not logged as Windows NTFS transactions; default unclean-volume recovery clears the Windows log. | E47 |
| macOS | anylinuxfs | **Partial** — Defaults to NTFS-3G and optionally uses ntfs3; both have write-journaling gaps described in the Linux NTFS/NTFS-3G assessments. NFS adds a documented default-driver I/O limitation. | E7 |
| macOS | NTFS4Mac | **Partial** — No named streams/xattrs or POSIX ownership mapping; recovery resets the log rather than implementing full Windows crash recovery. | E52 |
| macOS | CrossDriver Free | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| macOS | Paragon NTFS for Mac | **Coverage unresolved** (`Partial*`) — Vendor documents ordinary NTFS writes, but no sufficiently complete current benchmark for streams, permissions, compressed writes and crash recovery was located. No claim of full implementation is made. | E53 |
| macOS | Tuxera NTFS for Mac | **Coverage unresolved** (`Partial*`) — Vendor documents ordinary NTFS writes, but no sufficiently complete current benchmark for streams, permissions, compressed writes and crash recovery was located. No claim of full implementation is made. | E53 |
| macOS | iBoysoft NTFS for Mac | **Coverage unresolved** (`Partial*`) — Vendor documents ordinary NTFS writes, but no sufficiently complete current benchmark for streams, permissions, compressed writes and crash recovery was located. No claim of full implementation is made. | E53 |
| macOS | vmount Pro | **Coverage unresolved** (`Partial*`) — Formatting and ordinary mounts are advertised. The public product description does not establish the metadata, recovery and format-feature benchmark for each filesystem. | E13 |
| macOS | CrossDriver NTFS for Mac Pro | **Coverage unresolved** (`Partial*`) — Mounted access is documented, but complete ACL, metadata and on-disk feature coverage is not. Free/Pro and announced future capabilities must remain separate. | E19 |
| macOS | EaseUS NTFS for Mac | **Coverage unresolved** (`Partial*`) — Vendor documents ordinary NTFS writes, but no sufficiently complete current benchmark for streams, permissions, compressed writes and crash recovery was located. No claim of full implementation is made. | E53 |
| macOS | NTFSTool Pro | **Coverage unresolved** (`Partial*`) — Vendor documents ordinary NTFS writes, but no sufficiently complete current benchmark for streams, permissions, compressed writes and crash recovery was located. No claim of full implementation is made. | E53 |
| FreeBSD; OpenBSD | libfsntfs | **Largely implemented** — Reads current NTFS 3.0/3.1 including LZNT1/WOF compression and case-sensitive directories. EFS plaintext decryption is absent; that optional credential-dependent feature alone does not cross this benchmark’s threshold. | E54 |
| OpenBSD; DragonFly BSD | mount_ntfs | **Coverage unresolved** (`Partial*`) — Legacy-style native reader is documented; a complete current format-feature and metadata assessment is unavailable. Read-only mode alone does not establish Partial. | E55 |
| OpenBSD | ntfs-3g | **Partial** — Normal file/metadata support is broad, but writes are not logged as Windows NTFS transactions; default unclean-volume recovery clears the Windows log. | E47 |
| NetBSD | mount_ntfs | **Partial** — Writes only overwrite eligible existing data without create/delete/resize; native NTFS operations are substantially incomplete. | E55 |
| NetBSD | fuse-ntfs-3g + ntfsprogs | **Partial** — Normal file/metadata support is broad, but writes are not logged as Windows NTFS transactions; default unclean-volume recovery clears the Windows log. | E47 |
| DragonFly BSD | packaged mkntfs | **Largely implemented** — Upstream NTFS formatter works independently of the missing stock FUSE module. NM already records that this offering provides no mount. | E56 |
| Windows | Windows NTFS | **Largely implemented** — Reference Windows implementation; attached VHD/VHDX targets count as images. | E3 |

### exFAT

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux | exFAT + exfatprogs | **Largely implemented** — Reference Linux implementation and standard formatter; no important implementation gap identified for ordinary data-volume use. | E1 |
| Linux | exfat-fuse + exfat-utils | **Largely implemented** — Upstream exFAT driver and utilities cover normal exFAT file operations. Journaling, POSIX ACLs and hardlinks are not exFAT features, so their absence is not Partial. | E57 |
| Linux; Windows | FsRover | **Largely implemented** — Normal exFAT reading is documented; no important format-reader gap identified. R already records no writes. | E4 |
| macOS | Disk Utility / exFAT | **Largely implemented** — Reference macOS implementation; device and image workflows are documented. | E2 |
| macOS | vmount Pro | **Coverage unresolved** (`Partial*`) — Formatting and ordinary mounts are advertised. The public product description does not establish the metadata, recovery and format-feature benchmark for each filesystem. | E13 |
| FreeBSD | fusefs-exfat + formatter | **Largely implemented** — Upstream exFAT driver and utilities cover normal exFAT file operations. Journaling, POSIX ACLs and hardlinks are not exFAT features, so their absence is not Partial. | E57 |
| OpenBSD | exfat-fuse | **Largely implemented** — Upstream exFAT driver and utilities cover normal exFAT file operations. Journaling, POSIX ACLs and hardlinks are not exFAT features, so their absence is not Partial. | E57 |
| NetBSD | fuse-exfat | **Largely implemented** — Upstream exFAT driver and utilities cover normal exFAT file operations. Journaling, POSIX ACLs and hardlinks are not exFAT features, so their absence is not Partial. | E57 |
| DragonFly BSD | exfat-utils | **Largely implemented** — Normal exFAT formatting; the stock DragonFly FUSE-module absence concerns mounting and is already NM, not a formatter implementation defect. | E56 |
| Windows | Windows exFAT | **Largely implemented** — Reference Windows implementation; attached VHD/VHDX targets count as images. | E3 |

### ZFS / OpenZFS

| OS | Solution | Assessment | Evidence |
| --- | --- | --- | --- |
| Linux; FreeBSD | OpenZFS | **Largely implemented** — Current OpenZFS family covers core pool/dataset, integrity, snapshot, compression and encryption operations. Active feature flags still govern cross-version imports. | E58 |
| Linux; Windows | FsRover | **Partial** — Restricted pool-feature whitelist and no native dataset encryption support. | E4 |
| macOS | OpenZFS on OS X | **Largely implemented** — Current OpenZFS family covers core pool/dataset, integrity, snapshot, compression and encryption operations. Active feature flags still govern cross-version imports. | E58 |
| macOS | anylinuxfs | **Coverage unresolved** (`Partial*`) — Uses Linux drivers and OpenZFS through NFS. Broad file access is documented; preservation of each filesystem’s ACLs, xattrs and special operations through NFS is not established. | E6 |
| NetBSD | base ZFS | **Partial** — NetBSD documents that ZFS extended-attribute support is not wired up (PR 58556), an important mounted metadata gap independent of pool feature flags. | E59 |
| Windows | OpenZFS on Windows | **Coverage unresolved** (`Partial*`) — Prerelease port retains the OpenZFS engine. Windows ACL/xattr projection and recovery parity are not established by its release description; Beta records maturity separately. | E58 |
| Windows | DiskInternals Linux Reader Pro | **Coverage unresolved** (`Partial*`) — Drive-letter access is advertised. Complete metadata semantics and compatible feature profiles are not documented; file preview/export alone is excluded from the research bar. | E21 |

## Evidence

URLs below are upstream documentation/source or the product vendor. Version-specific
claims apply to the cited implementation; undocumented features remain unresolved.

### E1 — Linux native

- https://docs.kernel.org/filesystems/ext4/overview.html
- https://btrfs.readthedocs.io/en/latest/Introduction.html
- https://docs.kernel.org/filesystems/xfs/index.html
- https://github.com/exfatprogs/exfatprogs

### E2 — Apple formats

- https://support.apple.com/guide/disk-utility/dsku19ed921c/mac
- https://support.apple.com/guide/disk-utility/dskutl11888/mac

### E3 — Windows formats

- https://learn.microsoft.com/en-us/windows-server/storage/file-server/ntfs-overview
- https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/format
- https://learn.microsoft.com/en-us/windows-server/storage/disk-management/manage-virtual-hard-disks

### E4 — FsRover

- https://github.com/a1ive/FsRover/blob/master/FsRover/help.md
- https://github.com/a1ive/FsRover/blob/master/GtkRover/mount.cpp
- https://github.com/a1ive/FsRover/blob/master/FsRover/winfspfs.cpp

### E5 — xlinuxfs

- https://github.com/HuanchuanTech/xlinuxfs/blob/main/docs/filesystem-capabilities.md

### E6 — anylinuxfs

- https://github.com/nohajc/anylinuxfs

### E7 — anylinuxfs NTFS

- https://github.com/nohajc/anylinuxfs
- https://github.com/tuxera/ntfs-3g/blob/edge/ntfsprogs/ntfsrecover.8.in
- https://docs.kernel.org/filesystems/ntfs3.html

### E8 — PolyDrive

- https://polydrive.net/disk/

### E9 — ExtendFS

- https://apps.kpchew.com/extendfs/known-limitations

### E10 — Ext4Kit

- https://github.com/rayhanadev/Ext4Kit

### E11 — open_ext4_for_mac

- https://github.com/h3ct0r/open_ext4_for_mac

### E12 — Paragon extFS Mac

- https://www.paragon-software.com/home/extfs-mac/
- https://kb.paragon-software.com/article/6289

### E13 — vmount

- https://www.vmount.app/

### E14 — BSD ext4

- https://man.freebsd.org/cgi/man.cgi?query=ext2fs&sektion=4
- https://github.com/openbsd/src/blob/master/sys/ufs/ext2fs/ext2fs.h
- https://github.com/NetBSD/src/blob/netbsd-11/sys/ufs/ext2fs/ext2fs.h
- https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ext2fs/ext2fs.h

### E15 — fuse-ext2

- https://github.com/alperakcan/fuse-ext2/blob/master/fuse-ext2/fuse-ext2.c
- https://github.com/alperakcan/fuse-ext2/blob/master/fuse-ext2/op_getxattr.c
- https://github.com/alperakcan/fuse-ext2

### E16 — FreeBSD LKL

- https://github.com/freebsd/freebsd-ports/blob/main/filesystems/lkl/Makefile
- https://www.kernel.org/doc/html/v4.16/filesystems/
- https://man7.org/linux/man-pages/man5/ext4.5.html
- https://man7.org/linux/man-pages/man8/mkfs.xfs.8.html

### E17 — Windows ext4 formatters

- https://www.diskinternals.com/ext4-formatter/
- https://www.diskgenius.com/manual/format-partition.php
- https://www.diskgenius.com/manual/operate-virtual-disk.php
- https://www.partitionwizard.com/partitionmanager/format-ext4-windows.html

### E18 — Ext4Fsd

- https://github.com/bobranten/Ext4Fsd

### E19 — CrossDriver

- https://www.crossdriver.com/product/linux-disk-for-windows
- https://fs.crossdriver.com/en/mac-disk/
- https://www.crossdriver.com/product/ntfs-new

### E20 — Paragon Linux Windows

- https://www.paragon-software.com/home/linuxfs-windows/
- https://download.paragon-software.com/doc/Paragon_Linux_File_Systems_Windows.pdf

### E21 — DiskInternals mounts

- https://www.diskinternals.com/linux-writer/
- https://www.diskinternals.com/linux-reader/

### E22 — btrfs4mac

- https://github.com/gereoffy/btrfs4mac

### E23 — WinBtrfs

- https://github.com/maharmstone/btrfs

### E24 — fuse-xfs

- https://github.com/karan-vk/fuse-xfs

### E25 — xfsprogs FreeBSD

- https://github.com/freebsd/freebsd-ports/tree/main/filesystems/xfsprogs

### E26 — libfsxfs

- https://github.com/libyal/libfsxfs

### E27 — apfs-fuse

- https://github.com/sgan81/apfs-fuse

### E28 — linux APFS

- https://github.com/linux-apfs/linux-apfs-rw
- https://github.com/linux-apfs/apfsprogs

### E29 — libfsapfs

- https://github.com/libyal/libfsapfs

### E30 — Paragon APFS Linux

- https://www.paragon-software.com/home/apfs-linux/

### E31 — winapfs

- https://github.com/christofilojohn/winapfs

### E32 — apfs_for_windows

- https://github.com/RandyNorthrup/apfs_for_windows

### E33 — Paragon APFS Windows

- https://www.paragon-software.com/home/apfs-windows/
- https://paragon-software.zendesk.com/hc/en-us/articles/16290128217745-APFS-for-Windows-Supported-Volumes

### E34 — MacDrive

- https://www.owc.com/solutions/macdrive

### E35 — Linux UFS

- https://docs.kernel.org/admin-guide/ufs.html
- https://github.com/torvalds/linux/blob/master/fs/ufs/inode.c
- https://github.com/torvalds/linux/blob/master/fs/ufs/namei.c

### E36 — UFS2Tool

- https://github.com/SvenGDK/UFS2Tool/tree/v4.1
- https://github.com/SvenGDK/UFS2Tool/blob/v4.1/Ufs2DokanOperations.cs#L97-L127
- https://github.com/SvenGDK/UFS2Tool/blob/v4.1/macOS.md

### E37 — FreeBSD UFS

- https://man.freebsd.org/cgi/man.cgi?query=newfs&sektion=8
- https://man.freebsd.org/cgi/man.cgi?query=tunefs&sektion=8

### E38 — OpenBSD FFS

- https://man.openbsd.org/newfs.8
- https://man.openbsd.org/mount.8

### E39 — NetBSD FFS

- https://man.netbsd.org/newfs.8
- https://wiki.netbsd.org/features/UFS2ea/

### E40 — DragonFly native

- https://www.dragonflybsd.org/docs/handbook/Installation/
- https://man.dragonflybsd.org/?command=hammer2&section=8
- https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/ufs/fs.h

### E41 — fuse-ufs

- https://github.com/realchonk/fuse-ufs

### E42 — portable makefs

- https://github.com/kusumi/makefs
- https://github.com/kusumi/makefs/blob/master/src/usr.sbin/makefs/ffs.c

### E43 — linux_hammer2

- https://github.com/jdmanring/linux_hammer2/blob/main/doc/CAPABILITIES_MATRIX.md
- https://github.com/jdmanring/linux_hammer2
- https://github.com/kusumi/hammer2-utils

### E44 — hammer2-fuse

- https://github.com/kusumi/hammer2-fuse
- https://github.com/kusumi/hammer2-fuse/blob/master/src/main.rs

### E45 — FreeBSD hammer2

- https://github.com/freebsd/freebsd-ports/tree/main/filesystems/hammer2
- https://github.com/kusumi/freebsd_hammer2

### E46 — Linux NTFS

- https://docs.kernel.org/filesystems/ntfs3.html
- https://docs.kernel.org/filesystems/ntfs.html
- https://github.com/torvalds/linux/blob/master/fs/ntfs/file.c

### E47 — NTFS-3G

- https://github.com/tuxera/ntfs-3g/wiki/Manual
- https://github.com/tuxera/ntfs-3g/blob/edge/ntfsprogs/ntfsrecover.8.in

### E48 — Paragon NTFS Linux

- https://www.paragon-software.com/home/ntfs-linux-professional/

### E49 — macOS NTFS

- https://support.apple.com/en-us/101830
- https://github.com/apple-oss-distributions/ntfs

### E50 — NTFSTool

- https://github.com/ntfstool/ntfstool
- https://ntfstool.com/security

### E51 — TT NTFS

- https://github.com/dr-kbadawi/ttntfs
- https://github.com/dr-kbadawi/ttntfs/blob/main/docs/TODO.md

### E52 — NTFS4Mac

- https://github.com/vahidzekic/ntfs4mac

### E53 — Retail NTFS

- https://www.paragon-software.com/home/ntfs-mac/
- https://ntfsformac.tuxera.com/
- https://iboysoft.com/ntfs-for-mac/
- https://toolbox.easeus.com/support/ntfsformac/
- https://ntfstool.com/external-drive-formats-mac
- https://ntfstool.com/security

### E54 — libfsntfs

- https://github.com/libyal/libfsntfs

### E55 — BSD NTFS

- https://man.openbsd.org/mount_ntfs.8
- https://man.netbsd.org/mount_ntfs.8
- https://man.dragonflybsd.org/?command=mount_ntfs&section=8

### E56 — DragonFly formatters

- https://github.com/DragonFlyBSD/DPorts/tree/master/filesystems/fusefs-ntfs
- https://github.com/DragonFlyBSD/DPorts/tree/master/filesystems/exfat-utils
- https://github.com/DragonFlyBSD/DragonFlyBSD/blob/v6.4.2/sys/vfs/Makefile

### E57 — exFAT FUSE

- https://github.com/relan/exfat
- https://docs.freebsd.org/en/books/handbook/filesystems/
- https://github.com/openbsd/ports/tree/master/sysutils/exfat-fuse
- https://github.com/NetBSD/pkgsrc/tree/trunk/filesystems/fuse-exfat

### E58 — OpenZFS

- https://github.com/openzfs/zfs
- https://openzfs.github.io/openzfs-docs/Basic%20Concepts/Pool%20Structure/Feature%20Flags.html
- https://www.openzfsonosx.org/
- https://github.com/openzfsonwindows/openzfs/releases

### E59 — NetBSD ZFS

- https://wiki.netbsd.org/zfs/
- https://gnats.netbsd.org/58556
