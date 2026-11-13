# Encryption methods for Lurker

Research date: 2026-11-12. Platforms: Windows, macOS, Linux, FreeBSD, NetBSD, DragonFly BSD, and OpenBSD. The companion `encryption-methods-matrix.csv` contains the same method records for filtering.

The CSV also records free-integration status, licensing notes and licensing references. Licensing eligibility does not override a method's maintenance or platform-support qualifications.

Lurker can provide useful native encryption management on each OS without requiring a shared format. The native integration paths are BitLocker on Windows, Apple encryption on macOS, LUKS2 on Linux, GELI on FreeBSD, CGD on NetBSD, dm-crypt/LUKS on DragonFly BSD, and softraid CRYPTO on OpenBSD. DragonFly's old bundled cryptsetup requires a separate security/version review before admission. File encryption, encrypted folders, and encrypted backups need different workflows from disk containers.

This inventory covers major general-purpose data-at-rest methods, plus encrypted archives, backups, and specialized file formats that could plausibly fit Lurker. It does not count AES, ChaCha20, KDFs, transport encryption, password managers, or every commercial frontend as separate storage methods. Tomb, SiriKali, Plasma Vault, cryptmount, and systemd-homed manage other backends; they are not additional encryption formats. Age and rage implement one format, as do gocryptfs and cppcryptfs. Separate rows for LUKS1/LUKS2 and EncFS/CryFS generations capture materially different compatibility or maturity.

## Selected scope and availability (2026-11-14)

✅ integrated; 🟦 picked for future integration; — outside picked scope, not proof of impossibility. The authoritative scope is `../DIRECTIVES/LLM/CIPHER.md`. Selection does not establish runtime or security qualification.

| Backend / format | macOS | Linux | FreeBSD | DragonFly BSD | NetBSD | OpenBSD |
| --- | --- | --- | --- | --- | --- | --- |
| Apple encrypted disk images | ✅ | — | — | — | — | — |
| LUKS2 | — | ✅ | — | — | — | — |
| VeraCrypt | ✅ | ✅ | 🟦 | — | — | 🟦 |
| CryFS stable 1.x | 🟦 | 🟦 | — | — | — | — |
| gocryptfs | 🟦 | 🟦 | 🟦 | — | — | — |
| GELI | — | — | 🟦 | — | — | — |
| CGD | — | — | — | — | 🟦 | — |
| softraid CRYPTO / RAID1C | — | — | — | — | — | 🟦 |
| PEFS | — | — | 🟦 | — | — | — |
| Cryptomator | 🟦 | 🟦 | — | — | — | — |
| LUKS1 | — | 🟦 | — | 🟦 | — | — |

The broader availability matrices below retain all research records. EncFS, age/rage, securefs, fscrypt, OpenZFS, plain dm-crypt, Loop-AES, TCG Opal, commercial-only backends, archives, backups, rclone crypt and database/Git-specific encryption remain outside picked scope. LUKS uses dm-crypt internally.

Qualification notes:

- **Apple:** FileVault is supported startup-volume protection, not an encrypted container. Lurker implements AES-256 disk images via hdiutil with unencrypted APFS inside; it does not implement FileVault/native APFS-encryption management. [S04 S05 S06 S07]
- **DragonFly:** native dm-crypt and documented LUKS1 support exist; they must remain visible despite pending security review. Bundled cryptsetup identifies itself as 1.1.2; check patches/profiles rather than assuming age proves insecurity. LUKS2 support is unverified. The checked GnuPG package also needs a supported replacement or verified backports. [S10 S11 S12 S44 S47 S94]
- **VeraCrypt:** upstream supports FreeBSD x86-64 14+ and OpenBSD x86-64 7.8+. NetBSD/DragonFly remain unselected porting experiments. Inner-filesystem support is independent of crypto-format support. [S13]
- **Folders:** gocryptfs has a historical audit, macOS beta and FreeBSD basic support; those OS paths still need validation. CryFS means maintained stable 1.x with recovery limitations, never the data-loss-warning 2.0 alpha. PEFS requires a matching kernel/module; securefs has narrower independent assurance. EncFS legacy is unmaintained and its Rust rewrite lacks sufficient assurance for new-vault admission. [S31 S34 S38 S39 S80 S82 S83 S108]
- **Other files:** qualify maintained package versions, use an authenticated CMS profile, and do not substitute system LibreSSL for the researched OpenSSL backend. ZFS port presence alone does not establish encryption support. Historical audits are evidence, not certification of all current code/platforms. [S25 S26 S49 S50]

### Implementation and porting decisions

- Keep per-OS adapters in shared core. Prefer OS tools for native storage; use libraries where their API solves a concrete integration problem. Linking does not remove kernel, driver or privilege requirements. Dynamic libraries remain runtime dependencies; static/bundled components require Lurker security updates. Package dependencies can be installed by the package manager rather than manually by users. [S135]
- Current **macOS**: native images need no VeraCrypt install; VeraCrypt operations require the separately installed app and matching FUSE runtime. **Linux privileged**: LUKS and VeraCrypt open/close use system cryptsetup; VeraCrypt creation needs installed VeraCrypt. **Linux unprivileged**: bundled modified VeraCrypt/LKL helpers replace that dependency; AUR recipes include them, plain Cargo builds do not. See `linux-userspace.md` and `../README.md`.
- **age vs rage:** interoperable Go/Rust implementations, retained as research only. Neither provides a writable encrypted directory: files/streams require separate encryption/decryption, and rage-mount exposes encrypted archives read-only. They do not fit Lurker's session-unlock storage workflow and are excluded from integration, including the Rust `age` library. [S40 S41 S136]
- **VeraCrypt NetBSD/DragonFly:** start console-only and file-only, reuse existing BSD code and preserve crypto/format logic. Build/self-tests → disposable-container interoperability → mount/write/unmount/reopen → failure/read-only/busy-mount tests. Expected work: build/OS detection, dependencies, FUSE, virtual disks, mount discovery and cleanup. NetBSD provides PUFFS/librefuse; that does not prove VeraCrypt compatibility. Initial investigation could take hours; reliable porting may take days/weeks. These are untested estimates, not promised schedules; other T cells need individual feasibility checks. [S137 S138 S139]
- **HAMMER/HAMMER2:** no qualified native filesystem-encryption backend. Use HAMMER2 above DragonFly's encrypted block mapping; it is an inner filesystem, not another encryption method. The current HAMMER2 utility has no encryption controls. [S12 S140]

## How to read the matrix

Platform codes describe the method or implementation, not an existing Lurker port:

| Code | Meaning |
| --- | --- |
| N | Native OS facility documented in current OS manuals or vendor documentation. |
| S | Explicit upstream platform support or distribution; this does not promise every optional feature. |
| P | An OS package or port exists; runtime support, package freshness, and security patches still need validation. |
| E | Upstream calls the support beta, alpha, experimental, basic, or a release candidate. |
| L | An identified OS port is explicitly unmaintained or disabled. Exclude that port from the maintained support shortlist. |
| U | Support for this specific implementation and OS is unverified. Do not advertise it. |
| — | No supported direct implementation identified for this OS in the cited scope. This does not rule out VMs, emulation, or future ports. |

Admission is independent of platform availability:

- **Core:** an established native facility or established implementation suitable for a primary integration investigation.
- **Optional:** maintained and useful for a distinct need, with qualifications that should keep it out of an automatic default.
- **Compatibility:** support existing data; prefer another method for new storage.
- **Specialized:** a valid format with a separate product workflow, such as backups or database encryption.
- **Watch:** active development or an interesting implementation, but insufficient maturity for the requested proven-only shortlist.
- **Reject:** fails the maintenance, security, or actual-encryption requirement for new storage.

These are product assessments, not security certifications. A recent release proves activity, not cryptographic correctness; an old audit does not cover subsequent rewrites or every platform. “Core” on a supported OS does not promote that implementation's E, L or U cells to proven support. Sources below are upstream projects, OS manuals, vendor documentation, or official package sources. Package presence alone is deliberately labeled P and does not establish proven operation on that OS.

## Platform matrix

### Native disks and file containers

| ID | Method | Windows | macOS | Linux | FreeBSD | NetBSD | DragonFly BSD | OpenBSD | Admission | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D01 | BitLocker including Device Encryption and BitLocker To Go | N | — | U | U | — | — | — | Core | S01 S02 |
| D02 | Windows Encrypting File System EFS | N | — | — | — | — | — | — | Optional | S03 |
| D03 | APFS encryption and FileVault | — | N | U | — | — | — | — | Core | S04 S05 S06 |
| D04 | Apple encrypted disk images | — | N | U | — | — | — | — | Core | S07 |
| D05 | dm-crypt with LUKS2 | — | — | N | U | U | U | — | Core | S08 S09 |
| D06 | dm-crypt with LUKS1 | — | — | N | U | U | N | — | Compatibility | S08 S09 S10 S11 S93 S94 |
| D07 | Plain dm-crypt without a LUKS header | — | — | N | — | — | N | — | Optional | S08 S11 S12 |
| D08 | VeraCrypt | S | S | S | S | U | U | S | Core | S13 S14 S15 |
| D09 | FreeBSD GELI | — | — | — | N | — | — | — | Core | S16 S17 |
| D10 | NetBSD CGD | — | — | — | — | N | — | — | Core | S18 S19 S20 |
| D11 | OpenBSD softraid CRYPTO and RAID1C | — | — | — | — | — | — | N | Core | S21 S22 S23 S24 |
| D12 | OpenZFS native dataset and zvol encryption | E | U | S | N | U | U | — | Core | S25 S26 S27 S28 |
| D13 | Self-encrypting drives managed through TCG Opal or Windows encrypted-drive support | N | U | S | U | U | U | U | Optional | S111 S112 S113 |

Third-party readers of BitLocker, APFS, LUKS, and Apple images are not equivalent to an OS-native writable lifecycle. U cells do not establish read/write mounting. Linux libguestfs can expose LUKS through an appliance and FUSE without host-root operation after setup; it is an alternate implementation route for LUKS, not a new format. [S57]

VeraCrypt does not officially support every BSD. Its current list includes FreeBSD x86-64 starting at version 14 and OpenBSD x86-64 starting at version 7.8. NetBSD and DragonFly BSD are not listed; possible unofficial ports must not be advertised as supported without separate validation. [S13]

### Encrypted directories

| ID | Method | Windows | macOS | Linux | FreeBSD | NetBSD | DragonFly BSD | OpenBSD | Admission | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| F01 | Linux native filesystem encryption managed with fscrypt | — | — | N | — | — | — | — | Core | S29 S30 |
| F02 | gocryptfs format through gocryptfs or cppcryptfs | S | E | S | E | U | U | U | Core | S31 S32 S33 S110 |
| F03 | securefs | S | S | S | S | U | U | U | Optional | S34 |
| F04 | Cryptomator | S | S | S | U | U | U | U | Core | S35 S36 S37 |
| F05 | CryFS stable 1.x | E | S | S | U | U | U | U | Optional | S38 S39 |
| F06 | rclone crypt | S | S | S | S | S | P | S | Optional | S89 S90 S91 S92 |
| F07 | FreeBSD PEFS | — | — | — | P | — | — | — | Optional | S82 S83 S95 S108 |

### Files and archives

| ID | Method | Windows | macOS | Linux | FreeBSD | NetBSD | DragonFly BSD | OpenBSD | Admission | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| T01 | age format through age or rage | S | S | S | S | P | P | P | Core | S40 S41 S42 S43 |
| T02 | OpenPGP through GnuPG | S | S | S | P | P | P | P | Core | S44 S45 S46 S47 S48 S96 |
| T03 | CMS through OpenSSL | S | S | S | P | P | P | P | Optional | S49 S50 S97 S98 S99 S100 S101 S102 |
| T04 | scrypt file format and utility | U | P | P | P | P | L | P | Optional | S51 S52 S53 S54 S103 S104 S105 S109 S127 |
| T05 | AES Crypt stream format | S | S | S | U | U | U | U | Optional | S55 S56 |
| A01 | 7z AES encryption through current 7-Zip | S | S | S | P | P | P | P | Optional | S58 S59 S60 S61 S106 S114 |
| A02 | ZIP AES AE-1 and AE-2 | S | S | S | P | P | P | P | Compatibility | S58 S60 S61 S62 S106 S114 |
| A03 | RAR5 encryption through WinRAR or RAR | S | S | S | S | U | U | U | Optional | S63 |

T01's NetBSD and DragonFly entries refer to the age program's package sources, not verified rage ports. T03's BSD P cells refer to OpenSSL packages and must not silently substitute the system LibreSSL command. The archive BSD P cells require the current 7-Zip port; an obsolete p7zip package is not equivalent. AES Crypt advertises BSD generally, but does not establish support for each BSD variant, hence U rather than a blanket BSD claim.

### Backups and specialized encryption

| ID | Method | Windows | macOS | Linux | FreeBSD | NetBSD | DragonFly BSD | OpenBSD | Admission | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B01 | restic repository encryption | S | S | S | S | P | L | S | Specialized | S64 S65 S116 S117 S125 |
| B02 | BorgBackup repository encryption | E | S | S | S | P | U | P | Specialized | S66 S67 |
| B03 | Kopia repository encryption | S | S | S | S | U | L | S | Specialized | S68 S69 S118 S126 |
| B04 | Tarsnap encrypted backups | E | S | S | S | S | U | S | Specialized | S70 |
| X01 | SOPS structured file encryption | S | S | S | P | P | U | P | Specialized | S71 S119 S120 S121 |
| X02 | SQLCipher database encryption | S | S | S | P | U | P | P | Specialized | S72 S107 S122 S123 S124 |
| X03 | git-crypt repository file encryption | U | S | S | U | U | U | U | Specialized | S73 |
| C01 | BestCrypt Container Encryption | S | S | S | — | — | — | — | Optional | S74 S75 |
| C02 | DiskCryptor disk encryption | S | — | — | — | — | — | — | Optional | S76 |

Borg and Tarsnap Windows entries mean experimental or subsystem operation, not native Windows support. Backup support describes backup and restore; FUSE browsing and snapshot capture have separate platform requirements. The repository must be created with encryption enabled where the tool permits an unencrypted mode. SQLCipher is for applications using its SQLite implementation; it does not transparently encrypt arbitrary existing documents. SOPS composes established encryption and key services but has its own document envelope and editing workflow. [S66 S67 S70 S71 S72]

DragonFly's restic port explicitly sets `DFLY_UNMAINTAINED=yes`; its Kopia and scrypt ports explicitly set `IGNORE` for being unmaintained. These are L, not supported installation recommendations, even though their upstream projects are maintained. The observed DPorts GnuPG recipe is 2.4.7 while upstream lists 2.4 as past its June 2026 end of life; its P cell requires supported replacement or documented security backports before product use. [S44 S47 S125 S126 S127]

## Integration and security details

The CLI/API entries below identify integration surfaces, not complete runnable commands. Lurker should manage the installed implementation or use an appropriate maintained library; this research does not propose implementing new cryptography.

### Native disks and containers

| ID | Stored form and unlocked behavior | CLI or API | Privileges and prerequisites | Maintenance and maturity evidence | Product assessment and recovery requirements |
| --- | --- | --- | --- | --- | --- |
| D01 | Encrypted physical or virtual disk volume; unlocked as a Windows volume/drive. BitLocker To Go covers removable media. Device Encryption is the same BitLocker backend with a different provisioning path. | BitLocker PowerShell, Win32_EncryptableVolume WMI, manage-bde; Virtual Disk APIs for VHD/VHDX. | Elevation for provisioning and policy operations. Edition and device eligibility differ: full BitLocker management is not equivalent to Home Device Encryption. | Current Microsoft-supported OS facility, including current Windows client/server documentation. | Primary Windows disk integration. A VHDX encrypted with BitLocker is a container workflow, not a new cipher format. Detect conversion, protection and suspended states separately; preserve recovery protectors. Ordinary XTS/CBC encryption is not authenticated file-content encryption. |
| D02 | Individual NTFS files remain in place; account/certificate keys transparently decrypt authorized reads. No vault mount. | Windows EFS APIs; cipher.exe for operations and certificate backup. | Users can encrypt their writable files when EFS, NTFS, edition and policy permit. Administrator rights do not substitute for the user's private key. | Current Microsoft EFS/cipher documentation; long-deployed OS feature. | Useful account-integrated folder/file protection, not a portable password vault. Export certificate plus private key and handle recovery agents; copying to an unsupported filesystem may decrypt data. |
| D03 | Encrypted APFS volumes. FileVault adds startup-volume credential, recovery and boot integration; it is not a separate container format. | diskutil APFS operations; fdesetup for supported FileVault status/management operations; native authorization/settings or MDM where required. | Disk modification requires appropriate authorization; FileVault users require secure tokens and on Apple silicon volume ownership. Boot encryption is not merely an attach/detach action. | Current Apple Platform Security and deployment documentation; production OS facility. | Primary macOS native-volume support. Keep FileVault boot management separate from removable/data-volume unlock; preserve recovery keys and account ownership. Do not mistake hardware-only encryption before FileVault enablement for equivalent user-password protection. |
| D04 | Encrypted DMG, sparseimage or sparsebundle virtual disk; attachment exposes its internal filesystem. A sparsebundle is a directory of bands, not one file. | hdiutil create/attach/detach/info. | User-session image attachment is available; filesystem ownership and image operations still affect authorization. | Current Disk Utility image documentation and existing Lurker hdiutil integration. | Primary macOS container workflow. Disk-image encryption is distinct from native APFS encryption inside the image. Preserve the entire sparsebundle when using that representation. |
| D05 | LUKS2 header/keyslots plus encrypted block payload on a partition or file-backed device; unlock creates a mapper, followed by a separate filesystem mount. | cryptsetup luksFormat/open/close/luksDump/luksAddKey/luksKillSlot; libcryptsetup. | Standard Linux mappings and mounts need root/capabilities or an authorized service. Header operations on accessible regular files need not require root. | Active cryptsetup releases; current kernel dm-crypt documentation; mature Linux deployment. | Primary Linux disk/container backend. Preserve header backups and surviving credentials. XTS alone provides confidentiality, not authenticated content; authenticated dm-crypt/integrity configurations are a separate qualified profile, not the default. |
| D06 | LUKS1 header/keyslots plus encrypted block payload; mapper and filesystem mount are separate. | Linux cryptsetup; DragonFly bundled cryptsetup. | Privileged mapping/mounting; file containers additionally need a loop/vnode-backed device on the conventional route. | LUKS1 remains supported upstream. DragonFly documents native LUKS, but its source identifies the imported cryptsetup as 1.1.2; OS patches and security maintenance require review. | Open existing LUKS1 and consider it for DragonFly's native path only after version/security checks. Prefer LUKS2 for new Linux storage. DragonFly's bundled implementation is not evidence of LUKS2 or Argon2 support. |
| D07 | Raw encrypted block payload without self-describing LUKS metadata; mapper then filesystem mount. | cryptsetup plain mapping / dmsetup; DragonFly compatible dm target. | Privileged; exact cipher, key, IV, offset and geometry parameters must be preserved externally. | Maintained kernel/device-mapper interfaces; documented on Linux and DragonFly. | Niche interoperability or explicitly managed ephemeral storage. Poor easy-use default: no header to remember the settings, and no general built-in password verification or keyslots. |
| D08 | VeraCrypt container file or device; unlocked block image plus mounted filesystem. | VeraCrypt CLI; Linux cryptsetup TCRYPT/--veracrypt for opening supported volumes. | Windows normal-user file-container use after administrator driver installation. Stock Unix mapping/mounting commonly uses elevation; FUSE alone does not remove the filesystem-mount privilege requirement. | Current official OS list includes FreeBSD x86-64 14+ and OpenBSD x86-64 7.8+; established project with published security documentation. | Strong container option. Confirm actual inner-filesystem support per OS. No content authentication. Lurker's existing Linux unprivileged patched-helper/LKL route is a separate tested implementation, not a stock VeraCrypt guarantee on every OS. |
| D09 | GELI-encrypted GEOM provider with metadata; physical disk/partition or mdconfig-backed file. Exposes decrypted provider, then mounts UFS/ZFS/etc. | geli init/attach/detach/setkey/backup/restore; mdconfig for file-backed disks. | Root or a carefully scoped privileged helper for GEOM, vnode disk and mount operations. | Current FreeBSD GELI and mdconfig manuals; established native encryption supporting root partitions and metadata backup. | Primary FreeBSD disk/container candidate. AES-XTS is available; optional authentication is an additional profile. Preserve GELI metadata and all required passphrase/keyfile components; do not conflate GELI with ZFS dataset encryption. |
| D10 | CGD encrypted block device backed by a partition or vnd file; parameter file stores required configuration and salt; filesystem mount is separate. | cgdconfig; vnconfig/vnd; newfs and mount. | Privileged device setup/mounting. Parameters must accompany the ciphertext for recovery. | Native since NetBSD 2.0; AES-XTS since 8.0 and Adiantum since 10.0. Current cgdconfig also documents Argon2id; gate it by the installed release. | Primary NetBSD candidate. Use supported modern profiles and correct-key verification; wrong-key mappings can look like valid devices containing garbage. Preserve parameter files. Adiantum's wide-block protection is not an authentication tag. |
| D11 | softraid CRYPTO encrypted virtual disk; RAID1C adds mirrored ciphertext. A vnd-backed image can provide the backing disk for a container workflow. | bioctl; disklabel/newfs/mount; vnconfig for file-backed disk. | Privileged setup, activation and mounting. Passphrase files have strict ownership/mode requirements. | Current OpenBSD FAQ and bioctl/softraid manuals; native installer and external-drive workflows. | Primary OpenBSD disk/container candidate. Respect partition types, device identity and architecture-specific boot support. RAID1C adds redundancy, not a new encryption algorithm. Validate the vnd composition in an OpenBSD VM before release. |
| D12 | Encrypted ZFS datasets/zvols inside a ZFS pool. Each encryption root can have its own key; descendants may inherit it. | zfs create/load-key/unload-key/change-key/mount; zfs allow delegation. | Pool/device setup requires administration. Some key/dataset operations can be delegated; do not promise ordinary-user mounting uniformly across OSes. Requires ZFS. | Native OpenZFS encryption and maintained Linux/FreeBSD implementations; Windows port currently publishes release candidates. Other ports need independent encryption-feature validation. | Valuable for existing ZFS users. Not arbitrary-directory encryption on ext4/UFS/HAMMER2. AES-GCM/CCM authenticates encrypted data; pool structure, dataset/snapshot names and some metadata remain visible. Loading a key does not itself mount the dataset. |
| D13 | The physical drive encrypts sectors and enforces locking ranges; unlocked access remains a disk, not a new file container. | Linux cryptsetup LUKS2 Opal profiles; Windows encrypted-drive/BitLocker APIs; sedutil is another implementation requiring its own maintenance review. | Privileged device provisioning; suitable hardware, controller paths and firmware. Windows encrypted-drive eligibility includes requirements beyond generic Opal support. | Current cryptsetup documents --hw-opal and --hw-opal-only; Microsoft documents hardware-encrypted drive management and software-encryption policy defaults. | Hardware-specific optional integration, not a blanket proven default for every SED. Distinguish software-plus-hardware encryption from hardware-only protection. Qualify drive model/firmware and recovery/revert procedures; an always-encrypting drive without enforced authentication is not equivalent to a locked drive. |

### Directories

| ID | Stored form and unlocked behavior | CLI or API | Privileges and prerequisites | Maintenance and maturity evidence | Product assessment and recovery requirements |
| --- | --- | --- | --- | --- | --- |
| F01 | An encrypted directory tree on a supporting Linux filesystem; adding a key unlocks access in the existing filesystem, with no extra mount. | fscrypt setup/encrypt/unlock/lock/status; kernel fscrypt ioctls if needed. | Administrator filesystem/setup work first; user directory operations can then be unprivileged. Kernel docs list ext4, F2FS, UBIFS and CephFS; tool support must also be checked. | Current Linux kernel API and maintained fscrypt tool; widely deployed native filesystem-encryption design. | Strong Linux-only directory integration. Initialize empty directories and migrate existing data; policies inherit below them. Preserve policy/protector metadata; report busy files when key removal cannot fully lock access. Most non-filename metadata is exposed; no general content authentication. |
| F02 | Directory of encrypted files plus configuration and name metadata; mounted plaintext view. Windows implementation uses a virtual drive. | gocryptfs CLI and its documented stable CLI ABI; cppcryptfs documented command-line control on Windows. | User operation after FUSE or Dokany installation/setup. FUSE permissions differ by OS. | gocryptfs has a published 2017 audit and continuing releases. Version 2.7.0 adds basic FreeBSD support; upstream still describes macOS support as beta. cppcryptfs is an independent compatible Windows implementation. | High-value editable-folder feature. AES-GCM authenticates file blocks; file count, approximate sizes and tree relationships remain observable. Preserve config, directory IVs and long-name metadata. New gocryptfs options must be checked against cppcryptfs compatibility. |
| F03 | Encrypted directory; full and lite formats have different layout/metadata characteristics; FUSE/WinFsp plaintext view. | securefs create/mount/chpass and documented options. | User operation after platform filesystem-driver setup; Windows WinFsp, macOS macFUSE, Linux FUSE, documented FreeBSD build. | Established open-source implementation with maintained platform/build documentation; narrower independent assurance evidence than the primary defaults. | Useful alternative where its formats or platform behavior add value. Uses authenticated encryption. Treat full/lite format and optional size padding as real compatibility choices; preserve configuration. A generic upstream claim of BSD support does not validate all four BSDs. |
| F04 | Cryptomator vault directory with encrypted content and key/config files; virtual drive or local WebDAV view. | Official Cryptomator CLI can unlock/mount format-8 vaults; Java CryptoFS offers lower-level integration. | User operation after WinFsp/FUSE setup; macOS macFUSE or FUSE-T. WebDAV is a separate fallback with limitations. | Established maintained desktop project with documented authenticated storage architecture; official CLI exists but has a narrower surface than the desktop app. | Strong cloud-folder format. CLI unlock support does not establish a complete create/change-password/recovery API. Evaluate the supported Java interface for missing operations and AGPL/commercial licensing before integration; do not shell out to desktop UI automation. |
| F05 | Many encrypted fixed-size blocks plus config; mounted directory view with local integrity/rollback state. | Stable CryFS 1.x CLI; version-specific syntax and state directory. | User operation after FUSE setup on Linux/macOS; Windows remains experimental. | Stable branch is explicitly maintained separately from the 2.0 Rust alpha; stable documentation lists corruption conditions and no general repair tool. | Distinct metadata-concealment value: hides file sizes and directory structure. Optional rather than default due to recovery complexity. Enforce supported versions, free-space handling and non-concurrent access; preserve config and local state. |
| F06 | Encrypted files and names over a local or remote storage backend; copy/sync directly or expose an optional mounted view. | rclone crypt remote plus copy/sync/mount and remote-control API. | User operation for accessible storage; mount drivers and permissions are a separate per-OS requirement. | Maintained project with current upstream binaries for Windows/macOS/Linux/FreeBSD/NetBSD/OpenBSD and an official DragonFly port. | Useful local/cloud encryption with established automation. XSalsa20-Poly1305 authenticates contents; sizes and directory relationships leak. Preserve the password, optional salt and naming settings; rclone obscuring is not configuration encryption. Platform support for copy does not prove mount support. |
| F07 | FreeBSD kernel stacked encrypted filesystem over an existing directory, with multiple-key and PAM options; mounted plaintext view. | pefs mount/addkey and related keychain operations. | Kernel module and privileged mount/setup; later key operations depend on the configured policy. | Published 2014 audit plus current upstream maintenance: FreeBSD 15.1 rename changes and fixes/tests in July/August 2026. The homepage's 2018 release label is stale maintenance evidence. | A worthwhile FreeBSD-specific optional backend. AES/Camellia-XTS is not authenticated file encryption. Qualify the actual port/kernel pair and preserve keychain recovery material. Its historical audit does not cover every subsequent kernel adaptation. |

### Files and archives

| ID | Stored form and unlocked behavior | CLI or API | Privileges and prerequisites | Maintenance and maturity evidence | Product assessment and recovery requirements |
| --- | --- | --- | --- | --- | --- |
| T01 | One encrypted stream/file; decryption writes output. Directories need an explicit archive step. | age CLI/Go library or rage CLI/Rust age library. | Ordinary file permissions; no driver, mount or administrator access. | Maintained independent implementations, a published format, current upstream releases, and official BSD package sources. | Primary simple file encryption. Authenticated encryption with password or recipient keys. One user-facing format, not separate age/rage menu entries. rage-mount is read-only archive access, not a writable container backend. |
| T02 | OpenPGP encrypted message/file, optionally signed; decrypt to an output stream or file. | gpg with machine-readable status and passphrase channels; GPGME. | User operation; gpg-agent/pinentry and keyring lifecycle need deliberate handling. | Mature maintained GnuPG project and current official BSD ports; use a maintained branch rather than assuming old packaged versions remain supported. | Valuable interoperability and recipient-key workflow. Prefer modern authenticated profiles; distinguish recipient authenticity/signatures from ciphertext integrity. Do not parse human-readable output or automatically trust imported keys. |
| T03 | Standard CMS envelope; certificate/recipient or symmetric-key based decryption. | OpenSSL CMS CLI or maintained CMS/EVP APIs. | User operation; recipient private keys/certificates or explicitly managed symmetric credentials. | Maintained OpenSSL with documented CMS AEAD support; standard format with established deployment. | Useful PKI/interoperability option. AES-GCM is available; CBC-only envelopes are not automatically authenticated. Pin a supported profile and API/version; LibreSSL system commands need separate feature validation. |
| T04 | scrypt encrypted file format, not just the scrypt KDF used by other programs. | scrypt enc/dec/info. | User operation; memory/time caps must prevent pathological KDF demands. | Longstanding Colin Percival/Tarsnap implementation with current 1.3.x packages and maintained upstream source; the identified DragonFly port is explicitly disabled as unmaintained. | Valid small password-file alternative, but overlaps age for most users. Authenticated file construction; retain format compatibility. Do not repeat the project's historical comparison against old openssl enc defaults as a current benchmark. |
| T05 | AES Crypt .aes stream; current stream format 3, distinct from application version 4.x. | AES Crypt platform CLI or compatible maintained implementation. | User operation; packaging and redistribution conditions depend on the chosen implementation. | Maintained vendor format documentation; current format strengthens password KDF and authentication over older generations. | Useful .aes interoperability. Format 3 uses AES-CBC plus HMAC and a stronger KDF; some extension metadata is unauthenticated. Confirm per-BSD implementation rather than extrapolating from general BSD advertising. |
| A01 | One 7z archive containing documents; extraction produces plaintext files. Not a writable filesystem container. | Current upstream 7-Zip 7z/7zz CLI. | User operation; no filesystem driver. | Maintained upstream release series with native Windows/Linux/macOS tools and current BSD ports. | Convenient encrypted document bundle. Require encrypted headers to hide names. AES-256 encryption is not a general authenticated-encryption guarantee; CRC checks are not cryptographic authentication. |
| A02 | ZIP archive with WinZip AES extension; extracted files become plaintext. | Current 7-Zip or another explicitly AES-compatible archive implementation. | User operation; recipient software must support the AES extension. | Published AE-1/AE-2 specification and maintained archive implementations. | Interoperability mode. Require AES rather than weak traditional ZipCrypto. The extension authenticates encrypted file data but does not hide the ZIP directory/filenames; avoid treating it as equivalent to an encrypted filesystem. |
| A03 | RAR5 archive with optional encrypted headers; extraction produces plaintext. | Licensed RAR/WinRAR CLI; extraction-only implementations cannot create encrypted archives. | User operation; commercial redistribution/licensing needs review. | Active vendor releases for Windows/Linux/macOS/FreeBSD, with stable and beta channels. | Optional archive interoperability. Use a stable release and explicit RAR5 profile; do not infer authenticated-encryption guarantees solely from AES or archive checksums. |

### Backups and other specialized formats

| ID | Stored form and unlocked behavior | CLI or API | Privileges and prerequisites | Maintenance and maturity evidence | Product assessment and recovery requirements |
| --- | --- | --- | --- | --- | --- |
| B01 | Encrypted deduplicated backup repository with snapshots; restore/export, optional read-only browsing. | restic CLI and JSON output. | User operation for readable source files; full-system backup, snapshots and FUSE browsing have separate permissions. | Mature maintained stable releases and documented authenticated repository format; the identified DragonFly port is explicitly marked unmaintained. | Strong future backup feature on qualified OSes. It is not an editable vault. Preserve repository password/key records and test restore; AES-CTR plus Poly1305-AES authenticates stored objects. |
| B02 | Borg repository with encrypted authenticated archive chunks; restore or read-only browse. | Borg stable CLI, JSON/status features where documented. | User operation for readable files; filesystem snapshots, all-user backups and FUSE need additional access. | Mature maintained 1.4 stable line and security documentation; 2.0 beta is a separate compatibility decision. | Strong Unix backup option. Explicitly choose an encrypted repository mode. Do not market WSL as a native Windows backend or silently open a new-major repository with old tools. |
| B03 | Encrypted snapshot repository; restore/export or browsing. | kopia CLI, JSON and optional server API. | User operation for readable data; snapshot/driver requirements differ by OS. | Maintained releases and documented Windows/macOS/Linux/FreeBSD/OpenBSD distribution; the identified DragonFly port is explicitly disabled as unmaintained. | Useful backup alternative on qualified OSes. Specify authenticated repository algorithms and preserve recovery credentials. KopiaUI is not the integration target; Lurker can use the backend CLI. |
| B04 | Encrypted deduplicated backups in Tarsnap's service; restore through client. | tarsnap CLI. | User operation for readable sources; service account/billing and backup keys required. | Longstanding maintained security-focused client with explicit support for three BSDs, Linux and macOS. | Valid hosted-backup option, not a general local container. Windows documentation covers WSL. Keep service keys and recovery copies; avoid implying a local-only/offline repository backend. |
| X01 | Encrypted YAML/JSON/ENV/INI/binary envelope with key-service, age or PGP wrapping. | sops CLI and documented structured-file handling. | User operation plus any required KMS or recipient credentials. | Maintained CNCF project with established deployment in secrets workflows. | Developer-secrets feature only. It does not add a new disk format; field names may remain visible. Integrating editors requires plaintext temporary-file and process-lifecycle handling. |
| X02 | Encrypted SQLite database file; applications query through SQLCipher rather than a mount. | SQLCipher library/CLI and SQLite APIs. | User operation for owned databases; all readers must use a compatible SQLCipher build/profile. | Maintained established database implementation with published page-encryption/HMAC design. | Useful for Lurker's own protected data or explicit database workflows. AES-CBC with per-page HMAC; it cannot encrypt arbitrary applications' databases transparently without compatibility work. |
| X03 | Selected Git blobs encrypted on commit, plaintext working tree after unlock. | git-crypt init/unlock and Git filters. | User operation; Git and optional GnuPG key distribution. | Upstream 0.8.0 released in 2025; project explicitly says it has not reached maturity. | Specialized compatibility, not the proven default. Filenames/history metadata remain visible; revocation/key rotation limitations and Git-filter bypass risks differ from ordinary encrypted storage. |
| C01 | BestCrypt proprietary container with a virtual drive and chosen internal filesystem. | Vendor CLI, including Linux bctool; OS-specific SDK/automation surface needs validation. | Commercial license and installed platform drivers; verify provisioning and routine-use permissions per OS. | Established vendor product with current Windows/macOS/Linux offerings and documented CLI history. | Optional commercial interoperability. Review current driver support, licensing, recovery format and available automation before making a product promise; vendor marketing is not an independent security assessment. |
| C02 | DiskCryptor encrypted partition/system disk; Windows block-driver and boot integration. | DiskCryptor CLI and driver. | Administrator driver/disk operations; bootloader/Secure Boot setup is a separate deployment requirement. | Current maintained fork explicitly supports Windows 10/11, CLI, UEFI/GPT and ongoing compatibility maintenance. | Optional Windows disk alternative. A meaningful backend, but boot recovery and Windows update interactions increase support cost. Use data-volume support before attempting unattended system-disk management. |

## Methods to hold or exclude

These rows preserve the broader survey without presenting legacy or experimental software as satisfying “well built, well maintained, proven.” A hold is about the stated implementation/version, not a claim that its cryptographic primitive is broken.

| ID | Method | Windows | macOS | Linux | FreeBSD | NetBSD | DragonFly BSD | OpenBSD | Admission | Sources |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| H01 | OpenSSL enc raw symmetric files | S | S | S | P | P | P | P | Compatibility | S50 S97 S98 S99 S100 S101 S102 |
| H02 | Loop-AES | — | — | S | — | — | — | — | Optional | S77 S09 |
| H03 | Bcachefs native encryption | — | — | S | — | — | — | — | Watch | S78 S79 |
| H04 | EncFS original C++ 1.x | U | U | P | U | U | U | U | Compatibility | S80 |
| H05 | EncFS Rust rewrite and new V7 storage | — | E | E | E | U | U | U | Watch | S80 |
| H06 | CryFS Rust 2.0 rewrite | — | U | E | U | U | U | U | Watch | S38 |
| H07 | eCryptfs | — | — | P | — | — | — | — | Compatibility | S30 S81 |
| H09 | FreeBSD GBDE | — | — | — | — | — | — | — | Reject | S84 S115 |
| H10 | TrueCrypt format through tcplay | — | — | U | — | — | N | — | Compatibility | S10 S85 |
| H11 | OpenBSD vnconfig native Blowfish image encryption | — | — | — | — | — | — | N | Compatibility | S24 |
| H12 | LUKSbox .lbx containers | E | E | E | U | U | U | U | Watch | S86 S87 |
| H13 | VaultPony VeraCrypt implementation | S | S | S | U | U | U | U | Watch | S88 |
| H14 | Btrfs native per-directory encryption | — | — | U | — | — | — | — | Reject | S29 |

| ID | Stored form and unlocked behavior | CLI or API | Privileges and prerequisites | Maintenance and maturity evidence | Product assessment and recovery requirements |
| --- | --- | --- | --- | --- | --- |
| H01 | Encrypted stream with minimal header; decrypt writes output. | openssl enc and explicit cipher/KDF/digest settings. | User operation. | OpenSSL itself is actively maintained; enc deliberately does not support AEAD and the documentation recommends CMS for bulk encryption. | Existing-file compatibility only. Cipher/KDF choices are not fully self-describing and PBKDF2 does not add authentication. This is a limitation of enc, not of OpenSSL's library or CMS. Do not invent a Lurker format around raw EVP output. |
| H02 | Loop-AES encrypted device/file, often with externally managed key material. | Loop-AES tools/kernel integration or cryptsetup loopaes-compatible opening. | Root, kernel/loop integration and specialized tooling. | Upstream distributed updated Loop-AES sources in October 2026; it is inaccurate to label the project abandoned. | Maintained but niche. Little easy-use value over LUKS for new Linux users; consider only actual existing-volume demand. Preserve external parameters and key material. |
| H03 | Encryption integrated into a Bcachefs filesystem. | bcachefs tooling and kernel module. | Root/filesystem setup; currently maintained out of mainline as DKMS. | Active current project; old kernel repository archived because development moved to bcachefs-tools. | Defer broad product defaults pending filesystem/driver/recovery qualification. Active development is not evidence of equivalent operational maturity to established native defaults. Not fscrypt and not arbitrary-directory encryption on another filesystem. |
| H04 | Legacy encrypted directory and .encfs6.xml or older configuration; FUSE plaintext mount. | EncFS legacy CLI or compatible readers. | FUSE setup; user file operations afterward. | Upstream explicitly no longer maintains the old C++ code. Our local demo used EncFS 1.9.5, not the rewrite. | Existing-data access/migration only. Do not create new legacy-format vaults or treat old security findings as automatically fixed by changing the implementation language. |
| H05 | Legacy-compatible reads/writes plus a new V7 encrypted-directory format. | New encfsctl create/upgrade and encfs mount operations. | FUSE/macFUSE; per-OS support must match the new implementation. | Resumed development in 2026, but upstream explicitly labels the Rust implementation beta and recommends newer alternatives for new storage. | Watch rather than proven default. New authenticated blocks/config and stronger KDF are valuable; rewriting a config alone does not upgrade legacy file-data encryption. |
| H06 | CryFS block directory and version-specific local state. | CryFS 2 CLI. | Linux/FUSE; macOS untested and Windows unsupported in this branch. | Upstream calls it alpha and warns of data loss; stable 1.x is a separate maintained branch. | Exclude from production creation/mounting until qualified. Do not auto-upgrade a stable installation to the Rust branch. |
| H07 | Stacked encrypted directory mounted through the kernel. | ecryptfs-utils and mount.ecryptfs. | Privileged mount/setup; policy/helper configurations may delegate parts. | fscrypt upstream explicitly describes eCryptfs as no longer actively maintained; Ubuntu deprecated the encrypted-home implementation. | Legacy recovery/migration only, not a new-vault default. Not a single file container and not a substitute for fscrypt support on arbitrary OSes. |
| H09 | Legacy GEOM block encryption. | Historic gbde utility. | Privileged and release-dependent. | FreeBSD added a deprecation notice in 2023, scheduled removal for 15.0, and removed the source files in May 2024. | Reject for new storage; preserve a legacy recovery environment if importing actual old GBDE data. GELI is the relevant modern native candidate. |
| H10 | TrueCrypt-compatible container/device through DragonFly tcplay. | tcplay create/map/unmap/header operations. | Privileged device mapping and filesystem mounting. | Current DragonFly manual documents a bundled native implementation of the historical TrueCrypt format. | Compatibility only. TrueCrypt is discontinued. The tcplay manual's TrueCrypt claim is not proof of modern VeraCrypt compatibility, PIM support or current VeraCrypt KDFs. |
| H11 | Blowfish-encrypted vnd file image; separate PBKDF2 salt file with -K. | vnconfig -k/-K and filesystem tools. | Privileged vnd setup and mount. | Still documented in current OpenBSD vnconfig manual. | Native availability does not make an old 64-bit-block cipher a preferred new-storage profile. Prefer softraid CRYPTO over vnd; losing the external salt prevents recovery. |
| H12 | Custom .lbx container plus metadata mirror; FUSE/WinFsp view. | luksbox create/mount/umount and file operations. | User FUSE/WinFsp route after dependencies/configuration; hardware factors have extra device permissions. | Project advertises pre-v1 0.6.1 and says external audit and broader deployment remain future milestones. | Not on the proven shortlist. Despite its name, do not classify it as a LUKS1/LUKS2 implementation without format evidence. Research candidate only. |
| H13 | VeraCrypt-format containers through a new independent implementation and app. | Desktop/mobile app and shared Rust core; full CLI/system-mount contract not established. | Native user application; exported filesystem-mount behavior needs independent validation. | First desktop release in August 2026; compatibility assertions are not equivalent to the maturity of VeraCrypt itself. | Watch. An alternative implementation of an existing format, not a new encryption method; do not assume app browsing is a general writable OS mount. |
| H14 | No established deployable Btrfs-native directory-encryption backend in this survey. | None admitted. | Kernel and tool support would both be required. | Current kernel fscrypt supported-filesystem list excludes Btrfs; preliminary tools or patches are insufficient to establish a released backend. | Do not advertise as a feature. Btrfs on LUKS or a gocryptfs directory stored on Btrfs are supported compositions, not native Btrfs encryption. |

## Native priorities for each OS

These are broader research priorities, not selected implementation scope; the selected-scope matrix above takes precedence. They do not require an OS to read another OS's native format.

| OS | Disk or existing filesystem management | File containers | Folder protection | File encryption | Main integration constraint |
| --- | --- | --- | --- | --- | --- |
| Windows | BitLocker first; EFS as a distinct account-based feature | VeraCrypt; BitLocker on VHD/VHDX where edition and authorization permit | Cryptomator or gocryptfs through cppcryptfs; EFS on eligible NTFS systems | age; OpenPGP interoperability | Separate driver installation and administrative provisioning from routine user access; preserve BitLocker and EFS recovery material. |
| macOS | APFS data volumes; FileVault status and authorized management | Native encrypted disk images first; VeraCrypt optional | Cryptomator; securefs optional; gocryptfs remains beta | age; OpenPGP interoperability | FileVault boot/ownership semantics, macOS authorization and filesystem-driver installation are separate concerns. |
| Linux | LUKS2 first; OpenZFS for existing ZFS installations | LUKS2 or VeraCrypt; retain the existing explicit unprivileged implementation | gocryptfs; fscrypt where supported; Cryptomator optional | age; OpenPGP interoperability | Do not silently elevate an explicitly unprivileged request; validate kernel, filesystem and backend versions. |
| FreeBSD | GELI first; native OpenZFS dataset encryption | GELI on mdconfig-backed files; VeraCrypt on supported releases | ZFS datasets; PEFS or securefs optional; gocryptfs FreeBSD support is only basic | age; OpenPGP interoperability | GEOM/mdconfig privileges and OS-specific mount/state discovery; do not substitute Linux losetup or /proc assumptions. |
| NetBSD | CGD first | CGD on vnd-backed files | CGD-backed filesystem mounted at the desired directory; no fscrypt equivalent admitted | age from pkgsrc; OpenPGP or scrypt | Protect external CGD parameter files; gate KDFs by release and use correct-key verification. |
| DragonFly BSD | Native dm_target_crypt and bundled LUKS tooling, subject to security/version review | vnconfig-backed disk plus the supported LUKS mapping | Mount the encrypted native filesystem at the desired directory; no independent native folder backend admitted | age from DPorts; OpenPGP only with a supported build or verified backports | Validate bundled cryptsetup/header/cipher capabilities and package patch status. Identified scrypt/restic/Kopia ports are unmaintained; do not treat their directory entries as maintained support. |
| OpenBSD | softraid CRYPTO or RAID1C first | softraid over vnd-backed disk; VeraCrypt on officially supported releases | Use a mounted encrypted filesystem; no fscrypt equivalent admitted | age; OpenPGP or scrypt | bioctl/disklabel privileges, exact device identities, passphrase transport and filesystem support; avoid legacy Blowfish vnd creation. |

## Integration without paid backend licences

Lurker currently declares `GPL-3.0-only` in `Cargo.toml`. Native OS interfaces and separately installed open-source tools can provide a no-paid-backend integration path. This does not make every library directly link-compatible or grant unrestricted binary redistribution. Existing OS licences, eligible editions and hardware still apply; no enterprise licence is required merely because a feature has enterprise management options.

The following fee assessments describe integration through the OS interface or installed backend. They do not establish the right to relicense, embed or bundle every dependency. The broader inventory retains paid products for comparison; they are excluded from this shortlist.

| Methods | Free integration status | Conditions and sources |
| --- | --- | --- |
| BitLocker, BitLocker To Go, Device Encryption, EFS | Native on eligible Windows installations | No separate backend/enterprise integration licence. Full BitLocker management and EFS depend on Windows edition and policy; Home Device Encryption is narrower. [S01 S02 S03] |
| FileVault, APFS encryption, Apple encrypted disk images | Native on macOS | Use OS interfaces such as supported diskutil, hdiutil and FileVault management operations; native authorization and secure-token requirements remain. [S04 S05 S06 S07] |
| LUKS/dm-crypt, GELI, CGD, softraid, OpenZFS, fscrypt, PEFS | No paid backend licence for the installed-tool/native route | Retain each OS, maintenance and privilege qualification from the matrix. External command integration is distinct from copying or linking implementation code. [S08 S09 S16 S18 S21 S25 S30 S83] |
| VeraCrypt | No paid backend licence for external-tool integration | Observe its Apache/TrueCrypt-derived licence terms; do not treat it as GPL library code. macOS runtime distribution has separate conditions below. [S133] |
| gocryptfs/cppcryptfs, securefs, CryFS stable 1.x, rclone crypt | No paid backend licence for installed-tool integration | Preserve component licences and platform limitations. A macOS mounted view can introduce separate runtime conditions; rclone copy/sync encryption does not require a mount runtime. [S31 S32 S34 S39 S89] |
| Cryptomator CLI and CryptoFS | Free AGPLv3 route, conditional on compliance | A commercial licence is an alternative, not mandatory if the chosen integration satisfies the AGPL. Do not assume a linked combined application can remain GPL-only without evaluating the applicable terms. [S37 S134] |
| age/rage, GnuPG, OpenSSL CMS, scrypt, current 7-Zip | No paid backend licence for installed-tool integration | Distribution/component licence obligations remain. Use the supported maintained implementation on each OS. [S40 S41 S44 S49 S51 S58] |
| DiskCryptor | No paid backend licence for installed-tool integration | The maintained fork is GPLv3; driver installation, boot handling and Windows support still need qualification. [S76] |
| restic, BorgBackup, Kopia | No paid backend licence for local/self-managed storage | Paid storage providers are optional and separate. Do not include unmaintained DragonFly ports as supported merely because the upstream software is free. [S64 S66 S68] |
| BestCrypt | Paid product; excluded | A commercial container backend, not a no-paid-backend default. [S74] |
| Current AES Crypt desktop/server product | Paid product; excluded | Current vendor offering is a 30-day trial followed by a perpetual paid licence. The published .aes format and possible other implementations are separate questions; no alternative implementation was qualified here. [S55 S56] |
| RAR/WinRAR creation backend | Paid product; excluded | Trial use is limited to 40 days, followed by purchase. Free extraction does not provide a free RAR-creation backend. [S131] |
| Tarsnap service | Paid service; excluded | The service charges for storage and bandwidth. Free client availability does not make the backup service free. [S132] |

### Filesystem runtime licences

| Runtime | Free integration route | Distribution restriction |
| --- | --- | --- |
| macFUSE | Work with a separately user-installed runtime under its terms | Its licence prohibits binary bundling with commercial software without prior written permission, including automated download/installation in that context. Do not assume a no-fee bundled commercial installer. [S128] |
| FUSE-T | Personal use is free; a separately installed compatible runtime is distinct from embedding it | The current site offers a commercial licence for embedding and shipping FUSE-T in a product. That bundled route is outside the no-paid-licence shortlist. [S129] |
| WinFsp | GPLv3 and its FLOSS exception provide an open-source route | Commercial licensing is an alternative. Lurker's chosen distribution must satisfy the free licence/exception rather than assuming the commercial licence is necessary. [S130] |

### Research shortlist by OS without paid backends

This records available options, not selected features; archives, backups and rclone crypt below remain outside the selected scope.

| OS | Disk or container | Folder or filesystem | File or archive | Backup |
| --- | --- | --- | --- | --- |
| Windows | BitLocker/BitLocker To Go on eligible editions; VeraCrypt; DiskCryptor | EFS on eligible editions; cppcryptfs; securefs; Cryptomator via AGPL; rclone crypt | age/rage; GnuPG; OpenSSL CMS; 7-Zip | restic; Kopia |
| macOS | FileVault; encrypted APFS; native encrypted disk images; VeraCrypt with a compatible user-installed runtime | securefs; Cryptomator via AGPL; CryFS stable 1.x; rclone crypt, subject to mount-runtime conditions | age/rage; GnuPG; OpenSSL CMS; scrypt; 7-Zip | restic; BorgBackup; Kopia |
| Linux | LUKS2/dm-crypt; VeraCrypt; plain dm-crypt or Loop-AES for specialized needs | OpenZFS; fscrypt; gocryptfs; securefs; Cryptomator via AGPL; CryFS stable 1.x; rclone crypt | age/rage; GnuPG; OpenSSL CMS; scrypt; 7-Zip | restic; BorgBackup; Kopia |
| FreeBSD | GELI and GELI file containers; VeraCrypt on supported releases | OpenZFS; PEFS; securefs; rclone crypt | age/rage; GnuPG; OpenSSL CMS; scrypt; 7-Zip | restic; BorgBackup; Kopia |
| NetBSD | CGD and CGD file containers | rclone crypt | age; GnuPG; OpenSSL CMS; scrypt; 7-Zip | BorgBackup; restic through pkgsrc |
| DragonFly BSD | Native dm-crypt/LUKS1 and file containers, only after security/version review | rclone crypt through DPorts | age; OpenSSL CMS; 7-Zip, subject to package qualification | No maintained option admitted from the checked ports |
| OpenBSD | softraid CRYPTO/RAID1C and file containers; VeraCrypt on supported releases | rclone crypt | age; GnuPG; OpenSSL CMS; scrypt; 7-Zip | restic; BorgBackup; Kopia |

This filtered list preserves the earlier runtime qualifications: macOS gocryptfs remains beta, FreeBSD gocryptfs support is basic, and unmaintained DragonFly scrypt/restic/Kopia ports are excluded. SOPS, SQLCipher and git-crypt retain their separate specialized assessments; licensing was not independently audited for those integrations in this follow-up.

## Implications for Lurker

Keep the shared CLI and desktop behavior in `lurker-core`, with backend-specific operations and capability discovery. Model a backend's capabilities explicitly: create, unlock, lock, encrypt-file, decrypt-file, list, change credential, recover metadata, and manage keys are not universally available or interchangeable. Both frontends should use the same implementation; the desktop should continue using the helper rather than invoking the CLI.

Separate **format**, **implementation**, **storage representation**, **internal filesystem**, and **required privilege**. A VeraCrypt file with Btrfs inside is not automatically usable on an OS whose VeraCrypt implementation cannot mount Btrfs. A GELI or LUKS file container uses the same encryption as its device-backed equivalent. An age file has no editable mount. EFS and fscrypt operate in an existing filesystem. ZFS key loading and dataset mounting are separate states.

Make elevation operation-specific. A normal user may read a header or encrypt an owned file even when creating a mapping requires a helper. Installed FUSE/WinFsp/Dokany support may enable routine user mounting but does not prove that driver installation, physical-device access, or all filesystem operations are unprivileged. Record the real state after partial failures and preserve recoverable sessions on busy unmounts.

Use task-oriented entry points: protect a disk, create a container, protect a folder, and encrypt files. Backups are outside the selected scope. Choose an appropriate default for the detected OS and task; existing-data compatibility can select another backend. Do not expose a list of cipher names as though they were equivalent storage products.

Before promoting a backend/OS pair, validate it on that OS: create/open/close, wrong credentials, reboot/session recovery, read-only behavior, credential changes, backup/restore of recovery metadata, interrupted writes, full storage, busy locks/unmounts, Unicode names, permissions and minimum dependency versions. Test destructive operations on disposable virtual disks. CLI/core portability and Slint/winit desktop portability need separate validation. Existing project directives still describe implemented Linux/macOS behavior; this document is research and does not claim the additional ports are implemented.

## Sources

URLs are primary references inspected during the survey. Current/default manual pages may describe a development release; version-sensitive features must be compared with the actual target release before implementation.

| ID | Primary reference | URL |
| --- | --- | --- |
| S01 | Microsoft BitLocker overview and Device Encryption distinctions | https://learn.microsoft.com/en-us/windows/security/operating-system-security/data-protection/bitlocker/ |
| S02 | Microsoft manage-bde command interface | https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/manage-bde |
| S03 | Microsoft cipher and EFS certificate/recovery operations | https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/cipher |
| S04 | Apple FileVault security architecture | https://support.apple.com/guide/security/volume-encryption-with-filevault-sec4c6dc1b6e/web |
| S05 | Apple encrypted storage devices in Disk Utility | https://support.apple.com/guide/disk-utility/encrypt-and-protect-a-storage-device-with-a-password-dskutl35612/mac |
| S06 | Apple FileVault deployment, secure tokens and ownership | https://support.apple.com/guide/deployment/manage-filevault-with-mdm-dep0a2cb7686/web |
| S07 | Apple disk-image creation and encryption | https://support.apple.com/guide/disk-utility/create-a-disk-image-dskutl11888/mac |
| S08 | Linux kernel dm-crypt documentation | https://www.kernel.org/doc/html/latest/admin-guide/device-mapper/dm-crypt.html |
| S09 | cryptsetup supported formats and current release | https://gitlab.com/cryptsetup/cryptsetup/-/blob/main/README.md |
| S10 | DragonFly native disk-encryption features | https://www.dragonflybsd.org/features/ |
| S11 | DragonFly bundled cryptsetup manual | https://man.dragonflybsd.org/?command=cryptsetup&section=8 |
| S12 | DragonFly dm_target_crypt manual | https://man.dragonflybsd.org/?command=dm_target_crypt&section=4 |
| S13 | VeraCrypt supported operating systems | https://veracrypt.io/en/Supported%20Operating%20Systems.html |
| S14 | VeraCrypt privileges and driver installation | https://veracrypt.io/en/Using%20VeraCrypt%20Without%20Administrator%20Privileges.html |
| S15 | VeraCrypt authenticity and integrity limitations | https://veracrypt.io/en/Authenticity%20and%20Integrity.html |
| S16 | FreeBSD GELI manual | https://man.freebsd.org/cgi/man.cgi?query=geli&sektion=8&format=html |
| S17 | FreeBSD mdconfig manual | https://man.freebsd.org/cgi/man.cgi?query=mdconfig&sektion=8&format=html |
| S18 | NetBSD CGD guide and file-backed composition | https://www.netbsd.org/docs/guide/en/chap-cgd.html |
| S19 | NetBSD cgdconfig manual | https://man.netbsd.org/cgdconfig.8 |
| S20 | NetBSD CGD algorithms and introduction versions | https://man.netbsd.org/cgd.4 |
| S21 | OpenBSD native disk-encryption FAQ | https://www.openbsd.org/faq/faq14.html#softraidCrypto |
| S22 | OpenBSD softraid disciplines and boot support | https://man.openbsd.org/softraid.4 |
| S23 | OpenBSD bioctl interface and passphrase files | https://man.openbsd.org/bioctl.8 |
| S24 | OpenBSD vnconfig image and legacy encryption options | https://man.openbsd.org/vnconfig |
| S25 | OpenZFS dataset key management and encryption boundaries | https://openzfs.github.io/openzfs-docs/man/master/8/zfs-load-key.8.html |
| S26 | OpenZFS encryption properties | https://openzfs.github.io/openzfs-docs/man/master/7/zfsprops.7.html |
| S27 | OpenZFS Linux and FreeBSD getting-started documentation | https://openzfs.github.io/openzfs-docs/Getting%20Started/index.html |
| S28 | OpenZFS Windows release-candidate status | https://github.com/openzfsonwindows/openzfs/releases |
| S29 | Linux kernel fscrypt supported filesystems and threat model | https://docs.kernel.org/filesystems/fscrypt.html |
| S30 | fscrypt tooling, prerequisites and eCryptfs status | https://github.com/google/fscrypt |
| S31 | gocryptfs platform status, audit and release history | https://github.com/rfjakob/gocryptfs |
| S32 | cppcryptfs Windows implementation and compatibility | https://github.com/bailey27/cppcryptfs |
| S33 | gocryptfs cryptographic format | https://nuetzlich.net/gocryptfs/forward_mode_crypto/ |
| S34 | securefs formats, platforms and usage | https://github.com/netheril96/securefs |
| S35 | Cryptomator maintained desktop implementation | https://github.com/cryptomator/cryptomator |
| S36 | Cryptomator storage security architecture | https://docs.cryptomator.org/security/architecture/ |
| S37 | Cryptomator CLI capabilities and licensing | https://github.com/cryptomator/cli |
| S38 | CryFS 2.0 alpha status and platform restrictions | https://github.com/cryfs/cryfs |
| S39 | CryFS stable 1.x support and corruption/recovery limits | https://github.com/cryfs/cryfs/tree/release/1.0 |
| S40 | age format, platforms and usage | https://github.com/FiloSottile/age |
| S41 | rage implementation, Rust library and read-only archive mounting | https://github.com/str4d/rage |
| S42 | NetBSD pkgsrc age recipe | https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/security/age/Makefile |
| S43 | DragonFly DPorts age recipe | https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/security/age/Makefile |
| S44 | GnuPG maintained releases and supported release branches | https://www.gnupg.org/download/index.html |
| S45 | GnuPG operational commands and machine-readable output | https://www.gnupg.org/documentation/manuals/gnupg/Operational-GPG-Commands.html |
| S46 | NetBSD pkgsrc GnuPG recipe | https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/security/gnupg2/Makefile |
| S47 | DragonFly DPorts GnuPG recipe | https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/security/gnupg/Makefile |
| S48 | OpenBSD GnuPG port recipe | https://raw.githubusercontent.com/openbsd/ports/master/security/gnupg/Makefile |
| S49 | OpenSSL CMS envelope and AEAD support | https://docs.openssl.org/master/man1/openssl-cms/ |
| S50 | OpenSSL enc format, parameter and AEAD limitations | https://docs.openssl.org/master/man1/openssl-enc/ |
| S51 | Tarsnap scrypt file-encryption utility | https://www.tarsnap.com/scrypt.html |
| S52 | NetBSD pkgsrc scrypt recipe | https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/security/scrypt/Makefile |
| S53 | DragonFly DPorts scrypt recipe | https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/security/scrypt/Makefile |
| S54 | OpenBSD scrypt port recipe | https://raw.githubusercontent.com/openbsd/ports/master/security/scrypt/Makefile |
| S55 | AES Crypt product and platform claims | https://www.aescrypt.com/ |
| S56 | AES Crypt stream format versions and authentication | https://www.aescrypt.com/aes_stream_format.html |
| S57 | libguestfs guestmount user-mode FUSE and encrypted-image access | https://libguestfs.org/guestmount.1.html |
| S58 | Current upstream 7-Zip platform downloads | https://7-zip.org/download.html |
| S59 | 7z format encryption | https://7-zip.org/7z.html |
| S60 | DragonFly DPorts current 7-Zip implementation recipe | https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/archivers/7-zip/Makefile |
| S61 | OpenBSD current 7-Zip implementation recipe | https://raw.githubusercontent.com/openbsd/ports/master/archivers/7zip/Makefile |
| S62 | WinZip AES ZIP AE-1 and AE-2 format specification | https://www.winzip.com/en/support/aes-encryption/ |
| S63 | WinRAR and RAR platform/release downloads | https://www.rarlab.com/download.htm |
| S64 | restic supported installation targets | https://restic.readthedocs.io/en/stable/020_installation.html |
| S65 | restic authenticated repository format | https://restic.readthedocs.io/en/stable/100_references.html |
| S66 | Borg stable installation and platform limitations | https://borgbackup.readthedocs.io/en/stable/installation.html |
| S67 | Borg stable authentication and encryption design | https://borgbackup.readthedocs.io/en/stable/internals/security.html |
| S68 | Kopia supported installations | https://kopia.io/docs/installation/ |
| S69 | Kopia encrypted backup features | https://kopia.io/docs/features/ |
| S70 | Tarsnap supported client platforms | https://www.tarsnap.com/download.html |
| S71 | SOPS formats, key services and maintenance | https://github.com/getsops/sops |
| S72 | SQLCipher encrypted database design | https://www.zetetic.net/sqlcipher/design/ |
| S73 | git-crypt maturity and limitations | https://github.com/AGWA/git-crypt |
| S74 | BestCrypt current container product and platforms | https://jetico.com/data-encryption/encrypt-files-bestcrypt-container-encryption/ |
| S75 | BestCrypt Linux driver and bctool interface history | https://jetico.com/news/bestcrypt-container-encryption-linux-v2-beta/ |
| S76 | Maintained DiskCryptor fork and supported Windows workflow | https://github.com/DiskCryptor/DiskCryptor |
| S77 | Loop-AES current source distribution dates | https://loop-aes.sourceforge.net/ |
| S78 | Current Bcachefs source and DKMS distribution | https://github.com/koverstreet/bcachefs-tools |
| S79 | Bcachefs native encryption and project status | https://bcachefs.org/ |
| S80 | EncFS old-code retirement and Rust beta status | https://github.com/vgough/encfs |
| S81 | Ubuntu eCryptfs deprecation | https://documentation.ubuntu.com/security/security-features/storage/encryption-file/ |
| S82 | PEFS current-release claim and audit history | https://pefs.io/ |
| S83 | PEFS upstream source | https://github.com/freebsd-pefs/pefs |
| S84 | Current FreeBSD source tree at the former gbde path | https://cgit.freebsd.org/src/tree/sbin/gbde |
| S85 | DragonFly tcplay TrueCrypt-format interface | https://man.dragonflybsd.org/?command=tcplay&section=8 |
| S86 | LUKSbox pre-v1 status and outstanding external-audit milestone | https://luksbox.penthertz.com/ |
| S87 | LUKSbox container and sidecar workflow | https://luksbox.penthertz.com/docs/getting-started/quickstart/ |
| S88 | VaultPony first desktop release and app capabilities | https://vaultpony.app/desktop |
| S89 | rclone crypt format, password/salt handling and authenticated content | https://rclone.org/crypt/ |
| S90 | rclone current platform distributions | https://rclone.org/downloads/ |
| S91 | NetBSD pkgsrc rclone recipe | https://raw.githubusercontent.com/NetBSD/pkgsrc/trunk/net/rclone/Makefile |
| S92 | DragonFly DPorts rclone recipe | https://raw.githubusercontent.com/DragonFlyBSD/DPorts/master/net/rclone/Makefile |
| S93 | DragonFly vnode-disk file-container composition | https://man.dragonflybsd.org/?command=vnconfig&section=8 |
| S94 | DragonFly imported cryptsetup version and local-patch policy | https://github.com/DragonFlyBSD/DragonFlyBSD/blob/master/contrib/cryptsetup/README.DRAGONFLY |
| S95 | PEFS current maintenance and FreeBSD 15.1 fixes | https://github.com/freebsd-pefs/pefs/commits/master/ |
| S96 | FreeBSD GnuPG port | https://github.com/freebsd/freebsd-ports/tree/main/security/gnupg |
| S97 | OpenSSL native Windows builds | https://github.com/openssl/openssl/blob/master/NOTES-WINDOWS.md |
| S98 | OpenSSL Unix platform builds | https://github.com/openssl/openssl/blob/master/NOTES-UNIX.md |
| S99 | FreeBSD OpenSSL port | https://github.com/freebsd/freebsd-ports/tree/main/security/openssl |
| S100 | NetBSD OpenSSL package | https://github.com/NetBSD/pkgsrc/tree/trunk/security/openssl |
| S101 | DragonFly OpenSSL port | https://github.com/DragonFlyBSD/DPorts/tree/master/security/openssl |
| S102 | OpenBSD OpenSSL port | https://github.com/openbsd/ports/tree/master/security/openssl/3.5 |
| S103 | FreeBSD scrypt port | https://github.com/freebsd/freebsd-ports/tree/main/security/scrypt |
| S104 | Homebrew scrypt formula | https://formulae.brew.sh/formula/scrypt |
| S105 | Arch Linux scrypt package | https://archlinux.org/packages/extra/x86_64/scrypt/ |
| S106 | FreeBSD current 7-Zip port | https://github.com/freebsd/freebsd-ports/tree/main/archivers/7-zip |
| S107 | SQLCipher supported platforms | https://www.zetetic.net/sqlcipher/ |
| S108 | FreeBSD PEFS kernel-module port | https://github.com/freebsd/freebsd-ports/tree/main/sysutils/pefs-kmod |
| S109 | scrypt authenticated stream format | https://github.com/Tarsnap/scrypt/blob/master/FORMAT |
| S110 | gocryptfs stable CLI integration contract | https://github.com/rfjakob/gocryptfs/blob/master/Documentation/CLI_ABI.md |
| S111 | cryptsetup upstream-authored manual including Opal profiles | https://man7.org/linux/man-pages/man8/cryptsetup-luksFormat.8.html |
| S112 | Microsoft encrypted-drive requirements and policy | https://learn.microsoft.com/en-us/windows/security/operating-system-security/data-protection/encrypted-hard-drive |
| S113 | sedutil scope and platform binaries | https://github.com/Drive-Trust-Alliance/sedutil |
| S114 | NetBSD current upstream 7-Zip package | https://github.com/NetBSD/pkgsrc/blob/trunk/archivers/7-zip/Makefile |
| S115 | FreeBSD GBDE deprecation and removal history | https://github.com/freebsd/freebsd-src/commits/main/sbin/gbde/ |
| S116 | NetBSD restic package | https://github.com/NetBSD/pkgsrc/tree/trunk/sysutils/restic |
| S117 | DragonFly restic port | https://github.com/DragonFlyBSD/DPorts/tree/master/sysutils/restic |
| S118 | DragonFly Kopia port | https://github.com/DragonFlyBSD/DPorts/tree/master/sysutils/kopia |
| S119 | FreeBSD SOPS port | https://github.com/freebsd/freebsd-ports/tree/main/security/sops |
| S120 | NetBSD SOPS package | https://github.com/NetBSD/pkgsrc/tree/trunk/security/sops |
| S121 | OpenBSD SOPS port | https://github.com/openbsd/ports/tree/master/security/sops |
| S122 | FreeBSD SQLCipher port | https://github.com/freebsd/freebsd-ports/tree/main/databases/sqlcipher |
| S123 | DragonFly SQLCipher port | https://github.com/DragonFlyBSD/DPorts/tree/master/databases/sqlcipher |
| S124 | OpenBSD SQLCipher port | https://github.com/openbsd/ports/tree/master/databases/sqlcipher |
| S125 | DragonFly restic unmaintained marker | https://github.com/DragonFlyBSD/DPorts/blob/master/sysutils/restic/Makefile.DragonFly |
| S126 | DragonFly Kopia disabled/unmaintained marker | https://github.com/DragonFlyBSD/DPorts/blob/master/sysutils/kopia/Makefile.DragonFly |
| S127 | DragonFly scrypt disabled/unmaintained marker | https://github.com/DragonFlyBSD/DPorts/blob/master/security/scrypt/Makefile.DragonFly |
| S128 | macFUSE licence and commercial bundling restriction | https://github.com/macfuse/macfuse/blob/release/macfuse/LICENSE.txt |
| S129 | FUSE-T personal-use and commercial embedding/shipping terms | https://www.fuse-t.org/#licensing |
| S130 | WinFsp GPLv3 and FLOSS exception | https://github.com/winfsp/winfsp/blob/master/License.txt |
| S131 | RAR and WinRAR trial and purchase terms | https://www.rarlab.com/license.htm |
| S132 | Tarsnap service pricing | https://www.tarsnap.com/ |
| S133 | VeraCrypt component and redistribution licence terms | https://github.com/veracrypt/VeraCrypt/blob/master/License.txt |
| S134 | CryptoFS free AGPL and alternative commercial licensing | https://github.com/cryptomator/cryptofs |
| S135 | libcryptsetup public API and device operations | https://raw.githubusercontent.com/mbroz/cryptsetup/master/lib/libcryptsetup.h |
| S136 | Rust age library API, interoperability and beta notice | https://docs.rs/age/latest/age/ |
| S137 | VeraCrypt upstream build requirements | https://raw.githubusercontent.com/veracrypt/VeraCrypt/master/src/Readme.txt |
| S138 | VeraCrypt OpenBSD virtual-disk and mount adapter | https://raw.githubusercontent.com/veracrypt/VeraCrypt/master/src/Core/Unix/OpenBSD/CoreOpenBSD.cpp |
| S139 | NetBSD PUFFS and FUSE compatibility | https://netbsd.org/docs/puffs/ |
| S140 | HAMMER2 management interface | https://man.dragonflybsd.org/?command=hammer2&section=8 |

ArchWiki's data-at-rest inventory was used to check coverage, not as the sole authority for an implementation's current support: https://wiki.archlinux.org/title/Data-at-rest_encryption . The older Ubuntu community encrypted-filesystems page now redirects to an archive notice, so it is not current maintenance evidence: https://wiki.ubuntu.com/EncryptedFilesystems .
