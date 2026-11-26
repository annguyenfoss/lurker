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

## UFS2 prerequisite follow-up and intermediate product evidence

The fresh raw kernel-UFS2 control also passed using the existing configured
write-enabled module. That observation is separate from stock Fedora and from
the FUSE route used over 4096-byte LUKS mappings. A scoped exact-kernel depmod
override now selects the inspected existing write-enabled module after reboot;
its contents are tracked in `FEDORA_UFS_PREREQUISITE.conf`.

The fresh upstream FUSE driver source is pinned to
`ee5c4a447da98220a11edb69a225d51379afc4fb`. Native tests exposed an incorrect
rename destination lookup and an omitted FUSE ABI feature. The host-only
`FEDORA_FUSE_UFS_NOREPLACE.patch` (SHA256
`c9d71d6a6c24f69b0e1d64653983ad9995e9fd8e7b0b3409dc8deabdc352dc13`)
fixes those two issues. Seven native rename/collision checks and the full raw
payload cycle pass. Cryptomator/UFS2 now completes its ordinary payload cycle.

The additional host-only `FEDORA_FUSE_UFS_BARRIER.patch` (SHA256
`292718aec4842afc10668c293dc5d3137abaad77bae49fed8e539be5325932a7`)
forwards file and directory fsync through the driver cache to the scoped
backing-file `File::sync_all`. Errors propagate to callers. The final driver
SHA256 is `e7c2e1686387c66c3b13310486cbd8120972d9be0d8a64f37d48e5a1dc45dd27`;
28 rufs unit tests pass. Native strace observations confirm a new backing-file
fsync for both client operations; injected EIO reaches the client and a retry
succeeds. Fresh full raw payload write/append/verify and normal reopens pass,
with no remaining mount. This proves the observed barrier forwarding, not an
exhaustive filesystem durability audit. An exploratory destroy-time sync caused
a daemon teardown/reopen race and was removed before this qualified patch.
The installed source commits are `1dd06ad` (rename) and `b9779da` (barriers).
These remain explicitly configured host prerequisites; upstream labels writing
experimental. No filesystem implementation is bundled into Lurker.

The intermediate exact `8714ef5` product physical sweep completes **29/30**;
only LUKS2/UFS1 retains the pre-change geometry rejection. All other cells pass
full real-payload normal close/reopen cycles. A narrowly scoped native VeraCrypt
Quick Format confirmation was added to the device harness; it responds only
for the verified scratch device, create, VeraCrypt and explicit force. Earlier
interrupted device attempts remain forensic evidence, not completed sweeps.
The final source qualification and reboot evidence remain pending.

## Product UFS1 geometry verification

Exact committed source `93e0630` was freshly archived, built on Fedora, and all
373 archive files rehashed unchanged after the build. Four new product cases
pass: AES and Twofish LUKS2/UFS1 containers, and AES and Twofish LUKS2/UFS1 on
the verified `lurker-fresh-fedora` scratch disk. Each executes create plus three
mount/payload/normal-close cycles, including append and a second new file.
All headers identify 512-byte sectors, AES or Twofish XTS plain64 and 64-byte
keys. Argon2id retains 1048576 KiB memory; actual calibrated time costs and CPU
counts are recorded in the dumped JSON. No KDF/cipher reduction was needed.
The existing configured write-enabled native UFS prerequisite remains required.

Private complete evidence is `device/ufs1-geometry-93e0630.tar.gz` and
`validation/ufs1-geometry-93e0630-summary.json`. The latter four-case proof is
separate from the earlier raw native-tool geometry experiment. The physical
controller is terminal with both cases passed and the scratch device closed.

## Final Linux sweep started

Final Linux core/CLI/helper source is frozen at
`ca7d2137fca741008648360e98acc06ce559de3d`. A fresh exact archive is built on
Fedora and staged separately at
`/usr/local/lib/lurker-native-fresh-20261127-fedora-ca7d213`. The final retained
60-cell container sweep uses the complete UFS2 driver prerequisite, the fresh
validated CryFS donor, bounded common payload workers and actual native
filesystem identity checks. The initial 57/60 count records operational payload
outcomes; its older harness did not prove every selected filesystem identity.
The final guarded matrix is the acceptance evidence and remains in progress.

The repaired CryFS engine independently passed UFS2 and exFAT downloaded payload
cycles on Fedora. A fresh repeat of all three directory engines on these two
backings passes 6/6, with no remaining mounts or cleanup errors. The first repeat
had two Cryptomator launch failures because private cached runtime files retained
0600 permissions when copied into a root-owned staging tree. Only that new
staged tree was made readable/executable as appropriate, with root ownership and
write restrictions preserved; the fresh six-case repeat then passed. Runtime
bytes and source identity were unchanged.

## Combined follow-up candidate

The `ca7d213` retained sweep completed 59/60 with no cleanup errors. Its sole
failure was Cryptomator's first payload write on a native NTFS-3g backing
(`EIO`). The failed backing image and copied logs remain retained. The Fedora
retention recipe still selected NTFS-3g; the shared Linux recipe already chooses
the available native kernel writer. The owned retention/reboot recipes now use
that same selector and record the selected driver. Fedora's stock `ntfs3`
module is present and loaded, and its independent writable controls pass.

A shared mount-admission follow-up checks both VFS and superblock read-only
flags. Final combined source is now
`576505d554d798422840a06322dfa93b899365bc`, including the updated desktop.
The completed `ca7d213` receipts remain separate historical batch evidence.
A fresh combined workspace build/package and a new guarded 60-cell sweep will
precede final device/reboot gates. The idle old-source device queue was stopped
before it executed any device step.

Public header metadata for all ten `ca7d213` LUKS2 containers is retained. The
requested profile arguments stayed unchanged. Nine headers calibrated Argon2id
at 1048576 KiB; UFS2 calibrated at 891632 KiB during host load. The native
`--pbkdf-memory` argument is a calibration limit, so the observed value is not
relabeled as 1 GiB. A quiet follow-up will retain a separate comparison; no
product flags are being changed to force a value. Compiler and PBKDF work will
be serialized on this 3.8-GiB VM.

## Verified RPM and capacity follow-up

The combined `576505d` workspace check, test and build pass; all 386 archived
source files rehash unchanged. The native RPM SHA256 is
`0dfc01a676f46fe6c35d1389ce133078c216eaaa9a23d880a5063c8016a22118`;
staging archive SHA256 is
`65576e6cf3af9d4ae2b2d2bfd46e10359e19df20b9b4f3d7794c01b0d942f3d4`.
The actual RPM was extracted, every package-manifest file verified, copied to
an administrator-controlled prefix, and verified again. This is extracted native
package qualification on a prepared host; the system package was not replaced.
The packaged desktop opened a real winit/software X11 window, and the complete
CLI/helper credential suite passes. All three directory engines pass on the
selected native NTFS3 backing. A separate quiet UFS2/LUKS2 downloaded-payload
control passes and records Argon2id memory 1048576 KiB/time 9; the earlier
891632-KiB observation remains separate.

The full 576 sweep reached 52 passes and eight create failures when the guest
root filesystem filled. LUKS logs explicitly report ENOSPC; VeraCrypt returned
an incorrect-size error in the same exhausted environment. Failed fixtures and
all original results are retained. The same package is retesting those eight
cases after capacity repair; current passes will be reconciled with the unchanged
52 successful case receipts, with the initial failures retained as history.

A separate 32-GiB qcow2 now supplies persistent fixture space, independently
identified by serial `lrkf-fedora-store` and ext4 UUID
`52bc440f-db46-4924-b708-1bd20bfc96c6`. The qualification scratch disk remains
serial `lurker-fresh-fedora`. After the sweep became terminal, no round mounts
or loop sources remained. `FEDORA_FIXTURE_STORE.py` verified 9563 entries
(26.7 GB logical, approximately 12 GiB allocated), file hashes, owners and modes
before/after a closed-source copy and private bind at the unchanged round path.
UUID mount and ordered bind configuration persist through reboot. The original
underlay was removed only after verification; all failed files remain in the
verified copy or their original quarantine. Root has 23 GiB free and the fixture
store initially had 20 GiB free, covering approximately 2 GiB of retries plus a
9-GiB device-snapshot reserve and 2-GiB margin. No live mapper/backing source was
moved, and no product or KDF arguments changed for this repair.
