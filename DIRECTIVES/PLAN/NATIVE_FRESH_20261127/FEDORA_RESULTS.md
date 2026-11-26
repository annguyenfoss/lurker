# Fedora fresh qualification — in progress

Host owner: `fedora`, exclusive `adjutant@fedora-core-01`. This report concerns
fresh execution only. Previous installed artifacts supply inspected backend and
host-tool inputs, never pass receipts. Windows and nested compatibility systems
are outside the round.

## Frozen inputs and host

- Product source: `8adcdcf6c7d7719b4e1df2098e1807e390fbbe0d`.
- Exact Git archive SHA256: `9dcfa043e7ebc527abfb31c211b2bc49a24247243cae02fd20b8ff090de4709d`.
- Filesystem payload harness: `c5feaec`; Linux coordinator forwarding: `88e279c`.
- Fedora Linux 45 Cloud, running kernel `7.3.4-300.fc45.x86_64`.
- Initial boot ID: `22554395-9739-4c4c-b0ab-a3e43be6180c`.
- `dnf upgrade --refresh` and prerequisite installation completed successfully.
  Cryptsetup upgraded to 2.8.9 and SELinux policy to 45.25. Running kernel is the
  newest installed kernel. `dnf needs-restarting -r` reported no reboot needed.
  SELinux remains Enforcing. No broad security-policy change was made.
- Fresh `cargo check --workspace`, `cargo test --workspace`, and workspace build
  passed with one Cargo worker. CLI/helper were also freshly built using an
  inspected warm Cargo cache to begin runtime work during the desktop test build.
- Staged CLI SHA256:
  `fc629e7cc01d1174b07f2b242f276de58a834bba5d942f2f6195b15b6472043e`.
- Staged helper SHA256:
  `4fafc77a4c057e791ad8db8c7663bb97da3bf37a5636e038a0bc47774431f8c0`.
- Reviewed binaries and harnesses are installed in the administrator-controlled
  `/usr/local/lib/lurker-native-fresh-20261127-fedora`. Backend inputs resolve to
  the existing administrator-controlled private backend tree; their hashes are
  retained separately. This is a staged-artifact run, not an installed-package
  qualification.

## Filesystem route assessment and native controls

Fresh native-tool container controls passed Create + Read/Write for all ten
requested filesystems: ext4, Btrfs, XFS, APFS, UFS1, UFS2, HAMMER2, NTFS, exFAT
and ZFS. Each copied the downloaded Bitcoin, Ethereum and Transformer PDFs plus
text, closed normally, reopened and checked hashes, appended text and created a
second file, then closed/reopened and checked again. Every write was gated by a
real OS mount. These observations are native controls, not Lurker passes.

Ordinary distribution facilities provide ext4, Btrfs, XFS, NTFS and exFAT.
Additional native host-local candidates are:

| Filesystem | Actual route and current eligibility evidence |
| --- | --- |
| APFS | Fresh external `linux-apfs-rw` module built/loaded at commit `6cf6de8ad6fa4cb634a700b5d0436d73ee01942c`, upstream commit date 2026-11-19; GPL. Existing `mkapfs` supplies formatting. Write support is upstream experimental and remains a limitation despite this representative pass. |
| UFS1 | Existing loaded external matching `ufs.ko` with write support, kernel 7.3.4, taint `OE`; stock Fedora kernel configuration has `CONFIG_UFS_FS_WRITE` unset. No custom kernel/module build was started this round for UFS. |
| UFS2 | Existing `fuse-ufs` 0.6.0 host-local FUSE driver and `makefs-freebsd` formatter. This is an independent filesystem driver, not a bundled Lurker filesystem stack. |
| HAMMER2 | Existing kernel-matched external Linux HAMMER2 module and `newfs_hammer2`; BSD/GPL module. The inspected source cache identifies 1.0.110. Native mount and payload lifecycle succeeded. |
| ZFS | Existing kernel-matched OpenZFS 2.4.99-1 source-built module and host utilities. Stable 2.4.4 is also present; the matching running module is the newer candidate. |

Upstream source/maintenance references (fresh API commit observations and actual
installed input hashes are retained in private evidence):

- https://github.com/linux-apfs/linux-apfs-rw
- https://github.com/linux-apfs/apfsprogs
- https://github.com/realchonk/fuse-ufs
- https://github.com/jdmanring/linux_hammer2
- https://github.com/kusumi/hammer2-utils
- https://github.com/openzfs/zfs

Quick native feature observations used `setfacl`, `chmod`, `chown`, `stat`,
`cp --reflink=always`, and applicable native property commands. Ownership/mode
changes persisted as `1300:1300 640` on every filesystem except exFAT, whose
native root mount retains synthetic `0:0 755`. ACL succeeded on ext4, Btrfs, XFS
and NTFS. ZFS initially had `acltype=off`; enabling `posixacl` on the disposable
pool made the ACL probe succeed. APFS, UFS1, UFS2, HAMMER2 and exFAT rejected the
ACL operation. Reflink succeeded on Btrfs, XFS, APFS and ZFS. A failed reflink
probe does not establish absence of underlying CoW; unprobed CoW properties
remain unknown. ext4 journal features and XFS metadata were captured; other
journal/CoW details remain explicitly unknown or not applicable in receipts.

## Applicability ledger

Private `applicability.json` has all 130 filesystem × requested-backend cells.
For each filesystem the applicable Linux candidates are LUKS1, LUKS2, VeraCrypt,
gocryptfs, stable CryFS and Cryptomator. Directory rows name their native backing
filesystem and have no selectable inner filesystem.

The other 70 cells are explicitly not applicable: Apple encrypted disk images
require macOS `hdiutil`; APFS encryption requires macOS `diskutil` and is APFS
only; GELI and PEFS require FreeBSD kernel facilities; softraid CRYPTO/RAID1C
require OpenBSD; CGD requires NetBSD. No compatible native Linux provider for
those system facilities has been established. Filesystem creation support alone
does not provide those encryption backends.

## Live work and evidence

Remote persistent root: `/home/adjutant/.cache/lurker/native-fresh-20261127`.
Private local evidence:
`target/goal-validation/native-fresh-20261127/hosts/fedora/`.

- `native-container/native.json`: first 10/10 native controls.
- `native-container-features/native.json`: fresh 10/10 with explicit ownership
  changes and scoped ZFS ACL retry.
- `product-container/matrix.json`: initial required matrix finished **57/60**.
  All 30 combinations were attempted for privileged inner filesystems and all 30
  directory/backing combinations. The three failures are LUKS2/UFS1, CryFS/UFS2,
  and CryFS/exFAT. Every other required cell passed the downloaded-payload cycle.
- Incidental LKL comparison attempts reject the old userspace helper. These
  are excluded comparison evidence and are outside native acceptance.
- `logs/backend-cache-validation.json`: newer VC creator/LKL cache verified
  against its original exact `16aa1f4` source; relevant native/build input diff
  from that source to `8adcdcf` is empty. gocryptfs, stable CryFS and Cryptomator
  cached artifacts validate directly against `8adcdcf`. Original envelopes stay
  unchanged. New backend input copy is administrator controlled. LKL will not
  receive additional comparison work this round.
- `logs/build.log`: workspace check, tests and build passed.
- `logs/credentials.log`: CLI/helper prompt and password/keyfile slot suite passed.
- `logs/desktop-window.log`: fresh real X11/winit software-rendered window passed,
  with frame evidence `/tmp/lurker-desktop-window-bs4lma9d/result.json`.
- `logs/updates.log`, `logs/inventory.log`, `logs/native-provenance.log`,
  `logs/backend-input-hashes.log`, `logs/candidate-fresh-upstreams.log` preserve
  host/update/input details.

No qcow2 has been attached or overwritten yet. All applicable container cases
precede scratch-device work. Native device runner verifies the unique scratch
serial and active children immediately before destructive product creation.
Native LUKS2/UFS1 research after the initial batch **passed** with explicit
512-byte sectors and unchanged approved AES-XTS/plain64, 512-bit key and Argon2id
cost requests (1048576 KiB, 5000 ms). Metadata confirms 512-byte sectors and
1048576 KiB Argon memory; all downloaded/append hashes matched after normal
reopens. This is geometry research, not an additional product pass.

Both CryFS failures report `publish new encrypted block: Operation not permitted`.
The bundled durability patch currently publishes new blocks through hard links.
A fresh native no-clobber rename control succeeds on exFAT, including an EEXIST
collision preserving both original files. UFS2/fuse-ufs refuses absent-target
`renameat2(RENAME_NOREPLACE)` with EINVAL; its collision probe preserves both
files. These findings have been sent to the shared Linux/core owners. No
unsupported publication/durability fallback has been accepted.

VeraCrypt AES, Serpent and Twofish password suites and binary-keyfile/Auto suites
all passed: six suites, 72 subcases, including wrong credentials, duplicate and
aliased sources, destination protection and busy close.

Additional fresh research assessed MIT-licensed fstool: encrypted DMG creation
and rekeying exist as library APIs, but its encrypted payload backend is read-only.
This does not establish Create + Read/Write. Historical `cgd-linux` is marked
Pre-Alpha with its last published files dated 2004-02-23. References:

- https://github.com/KarpelesLab/fstool#encryption
- https://sourceforge.net/projects/cgd-linux/files/
 A retained final-sweep wrapper and post-reboot verifier are
prepared; generated test-only credentials remain private mode0600 outside logs.
Device, reboot persistence, broader runtime conformance and final frozen-candidate
validation are pending; this report does not claim completion.
